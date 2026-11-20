//! The Model Context Protocol over stdio: newline-delimited JSON-RPC 2.0, tools only.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::tools::Tools;

/// Protocol revisions this server speaks, newest first. A client asking for one of them
/// gets it back; any other gets the newest.
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub struct Server {
    tools: Tools,
}

impl Server {
    pub fn new(tools: Tools) -> Server {
        Server { tools }
    }

    /// Serve until stdin closes.
    pub fn run(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle_line(&line) {
                let mut bytes = serde_json::to_vec(&reply).unwrap_or_default();
                bytes.push(b'\n');
                output.write_all(&bytes)?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// One line in, at most one line out (notifications get no answer).
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(error(Value::Null, PARSE_ERROR, &format!("parse error: {e}"))),
        };
        match msg {
            // Batches, from the 2025-03-26 revision.
            Value::Array(items) => {
                let replies: Vec<Value> = items.into_iter().filter_map(|m| self.handle(m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            msg => self.handle(msg),
        }
    }

    fn handle(&mut self, msg: Value) -> Option<Value> {
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // A response to something we never send, or junk.
            return msg
                .get("id")
                .filter(|_| msg.get("result").is_none() && msg.get("error").is_none())
                .map(|id| error(id.clone(), INVALID_REQUEST, "not a request"));
        };
        // Notifications (no id): `notifications/initialized`, `notifications/cancelled`…
        // Every call finishes before the next line is read, so there is nothing to cancel.
        let id = msg.get("id")?.clone();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => result(id, self.initialize(&params)),
            "ping" => result(id, json!({})),
            "tools/list" => result(id, json!({ "tools": self.tools.list() })),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(error(id, INVALID_PARAMS, "tools/call needs a tool name"));
                };
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                match self.tools.call(name, &args) {
                    Some(r) => result(id, r),
                    None => error(id, INVALID_PARAMS, &format!("unknown tool: {name}")),
                }
            }
            other => error(id, METHOD_NOT_FOUND, &format!("method not found: {other}")),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params.get("protocolVersion").and_then(Value::as_str);
        let version = PROTOCOL_VERSIONS
            .into_iter()
            .find(|v| Some(*v) == asked)
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "hecate-mcp",
                "title": "hecate",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": self.tools.instructions(),
        })
    }
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::hecate::Hecate;

    fn server(config: Config) -> Server {
        Server::new(Tools::new(
            config,
            Hecate::new(Some("/nonexistent/hecate.sock".into())),
        ))
    }

    fn call(s: &mut Server, msg: Value) -> Value {
        s.handle_line(&msg.to_string()).expect("a reply")
    }

    #[test]
    fn initialize_negotiates_the_version() {
        let mut s = server(Config::default());
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
        );
        assert_eq!(r["id"], 1);
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(r["result"]["serverInfo"]["name"], "hecate-mcp");
        assert!(r["result"]["capabilities"]["tools"].is_object());

        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}),
        );
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let mut s = server(Config::default());
        assert_eq!(
            s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None
        );
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":"a","method":"resources/list"}),
        );
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(r["id"], "a");
        let r = s.handle_line("{nope").unwrap();
        assert_eq!(r["error"]["code"], PARSE_ERROR);
        let r = call(&mut s, json!({"jsonrpc":"2.0","id":3,"method":"ping"}));
        assert_eq!(r["result"], json!({}));
    }

    #[test]
    fn batches_answer_each_request() {
        let mut s = server(Config::default());
        let r = s
            .handle_line(
                r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
            )
            .unwrap();
        assert_eq!(r.as_array().unwrap().len(), 2);
    }

    #[test]
    fn tools_list_and_unknown_tool() {
        let mut s = server(Config::default());
        let r = call(&mut s, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        let tools = r["result"]["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "list_windows"));
        for t in tools {
            assert!(t["inputSchema"]["type"] == "object", "{t}");
            assert!(t["description"].as_str().is_some_and(|d| !d.is_empty()));
        }
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"rm_rf","arguments":{}}}),
        );
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn a_compositor_that_is_not_there_is_a_tool_error() {
        let mut s = server(Config::default());
        let r = call(
            &mut s,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_windows"}}),
        );
        assert_eq!(r["result"]["isError"], true);
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("connecting to hecate"), "{text}");
    }

    #[test]
    fn run_reads_lines_until_eof() {
        let mut s = server(Config::default());
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
        let mut out = Vec::new();
        s.run(&input[..], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("\"id\":1"));
    }
}
