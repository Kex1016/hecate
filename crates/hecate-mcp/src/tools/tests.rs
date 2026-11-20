use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::*;

/// A pretend compositor: answers each request line with what `reply` says, and keeps the
/// requests.
struct FakeHecate {
    dir: PathBuf,
    socket: PathBuf,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeHecate {
    fn start(name: &str, reply: fn(&Value) -> Value) -> FakeHecate {
        let dir = std::env::temp_dir().join(format!("hecate-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("hecate.wayland-1.1.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut line = String::new();
                if BufReader::new(&stream).read_line(&mut line).is_err() {
                    continue;
                }
                let req: Value = serde_json::from_str(&line).unwrap();
                let out = reply(&req);
                seen.lock().unwrap().push(req);
                let _ = writeln!(stream, "{out}");
            }
        });
        FakeHecate {
            dir,
            socket,
            requests,
        }
    }

    fn tools(&self, config: Config) -> Tools {
        Tools::new(config, Hecate::new(Some(self.socket.clone())))
    }

    fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeHecate {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn window(id: u64, app_id: &str, title: &str, ws: u64) -> Value {
    json!({
        "id": id, "title": title, "app_id": app_id, "pid": 100 + id, "workspace_id": ws,
        "is_focused": id == 1, "is_floating": true, "is_urgent": false,
        "layout": {
            "window_size": [400, 300], "tile_size": [400.0, 300.0], "pos_in_scrolling_layout": null,
            "tile_pos_in_workspace_view": null, "window_offset_in_tile": [0.0, 0.0]
        },
        "output": "DP-1", "geometry": { "x": 10, "y": 20, "width": 400, "height": 300 },
        "stacking": 0, "is_maximized": false, "is_fullscreen": false, "is_minimized": false,
        "always_on_top": false, "always_on_bottom": false, "omnipresent": false, "is_x11": false,
        "focus_serial": id, "border_tint": if id == 2 { json!("gold") } else { Value::Null },
    })
}

fn compositor(req: &Value) -> Value {
    if req == "windows" {
        return json!({ "ok": [window(1, "kitty", "~/src/hecate", 1), window(2, "org.gnome.Nautilus", "Files", 2)] });
    }
    if req == "outputs" {
        return json!({ "ok": { "DP-1": {
            "name": "DP-1", "make": "Dell", "model": "U2720Q", "serial": null, "physical_size": [600, 340],
            "modes": [{ "width": 3840, "height": 2160, "refresh_rate": 60000, "is_preferred": true }],
            "current_mode": 0, "vrr_supported": false, "vrr_enabled": false,
            "logical": { "x": 0, "y": 0, "width": 1920, "height": 1080, "scale": 2.0, "transform": "Normal" },
            "is_focused": true, "usable_area": null
        } } });
    }
    if let Some(action) = req.get("action") {
        if action["action"]["border_tint"] == "mauve" {
            return json!({ "error": "unknown tint \"mauve\": gold, love, foam, none or a #rrggbb colour" });
        }
        return json!({ "ok": {} });
    }
    json!({ "error": "not in this fake" })
}

fn text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

fn names(tools: &Tools) -> Vec<String> {
    tools
        .list()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn read_only_unless_allowed() {
    let fake = FakeHecate::start("perm", compositor);
    let tools = fake.tools(Config::default());
    let listed = names(&tools);
    for read in [
        "list_windows",
        "get_window",
        "list_workspaces",
        "list_outputs",
        "list_notifications",
        "screenshot",
    ] {
        assert!(
            listed.contains(&read.to_string()),
            "{read} missing from {listed:?}"
        );
    }
    for write in [
        "focus_window",
        "move_window_to_workspace",
        "switch_workspace",
        "set_border_tint",
        "send_notification",
    ] {
        assert!(
            !listed.contains(&write.to_string()),
            "{write} listed without being allowed"
        );
    }
    for t in tools.list() {
        assert_eq!(t["annotations"]["readOnlyHint"], true, "{t}");
    }
    assert!(tools.instructions().contains("read-only"));

    // A hidden tool called anyway is refused, and hecate never hears of it.
    let r = tools
        .call("set_border_tint", &json!({ "color": "gold" }))
        .unwrap();
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("--allow tint"), "{}", text(&r));
    assert!(fake.requests().is_empty());

    let mut config = Config::default();
    config.allow.insert(Permission::Tint);
    config.screenshots = false;
    let tools = fake.tools(config);
    let listed = names(&tools);
    assert!(listed.contains(&"set_border_tint".to_string()));
    assert!(!listed.contains(&"focus_window".to_string()));
    assert!(!listed.contains(&"screenshot".to_string()));
    assert!(tools.call("screenshot", &json!({})).unwrap()["isError"] == true);
}

#[test]
fn list_windows_filters_and_reads_tints() {
    let fake = FakeHecate::start("windows", compositor);
    let tools = fake.tools(Config::default());
    let r = tools.call("list_windows", &json!({})).unwrap();
    assert_eq!(r["isError"], false);
    let windows = r["structuredContent"]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[1]["border_tint"], "gold");
    assert!(
        windows[0].get("layout").is_none(),
        "summaries leave niri's layout out"
    );
    // The text content is the same JSON, for clients that don't read structuredContent.
    let parsed: Value = serde_json::from_str(text(&r)).unwrap();
    assert_eq!(parsed, r["structuredContent"]);

    let r = tools
        .call("list_windows", &json!({ "app_id": "NAUTILUS" }))
        .unwrap();
    assert_eq!(r["structuredContent"]["windows"].as_array().unwrap().len(), 1);
    let r = tools.call("list_windows", &json!({ "workspace_id": 1 })).unwrap();
    assert_eq!(r["structuredContent"]["windows"][0]["app_id"], "kitty");
    let r = tools.call("list_windows", &json!({ "appid": "kitty" })).unwrap();
    assert_eq!(r["isError"], true, "a misspelt argument is refused, not ignored");
}

#[test]
fn set_border_tint_sends_the_action() {
    let fake = FakeHecate::start("tint", compositor);
    let mut config = Config::default();
    config.allow.insert(Permission::Tint);
    let tools = fake.tools(config);

    let r = tools
        .call("set_border_tint", &json!({ "color": "gold", "app_id": "kitty" }))
        .unwrap();
    assert_eq!(r["isError"], false, "{r}");
    let r = tools
        .call(
            "set_border_tint",
            &json!({ "color": "love", "ms": 3000, "id": 2 }),
        )
        .unwrap();
    assert_eq!(r["isError"], false, "{r}");
    let r = tools
        .call("set_border_tint", &json!({ "color": "mauve" }))
        .unwrap();
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("unknown tint"));
    let r = tools
        .call(
            "set_border_tint",
            &json!({ "color": "gold", "id": 1, "app_id": "kitty" }),
        )
        .unwrap();
    assert_eq!(r["isError"], true, "two selectors");
    let r = tools.call("set_border_tint", &json!({})).unwrap();
    assert!(text(&r).contains("needs \"color\""));

    assert_eq!(
        fake.requests(),
        vec![
            json!({ "action": { "action": { "border_tint": "gold" }, "target": { "app_id": "kitty" } } }),
            json!({ "action": { "action": { "border_tint": { "color": "love", "ms": 3000 } }, "target": { "id": 2 } } }),
            json!({ "action": { "action": { "border_tint": "mauve" } } }),
        ]
    );
}

#[test]
fn window_actions() {
    let fake = FakeHecate::start("actions", compositor);
    let mut config = Config::default();
    config.allow.insert(Permission::Windows);
    let tools = fake.tools(config);
    assert_eq!(
        tools.call("focus_window", &json!({ "id": 2 })).unwrap()["isError"],
        false
    );
    assert_eq!(tools.call("focus_window", &json!({})).unwrap()["isError"], true);
    let r = tools.call(
        "move_window_to_workspace",
        &json!({ "workspace": "3", "title": "Files" }),
    );
    assert_eq!(r.unwrap()["isError"], false);
    assert_eq!(
        tools
            .call("switch_workspace", &json!({ "workspace": "next" }))
            .unwrap()["isError"],
        false
    );
    assert_eq!(
        tools
            .call("switch_workspace", &json!({ "workspace": 0 }))
            .unwrap()["isError"],
        true
    );
    assert_eq!(
        fake.requests(),
        vec![
            json!({ "action": { "action": "focus", "target": { "id": 2 } } }),
            json!({ "action": { "action": { "move_to_workspace": 3 }, "target": { "title": "Files" } } }),
            json!({ "action": { "action": { "workspace": "next" } } }),
        ]
    );
}

#[test]
fn list_outputs_summary() {
    let fake = FakeHecate::start("outputs", compositor);
    let r = fake
        .tools(Config::default())
        .call("list_outputs", &json!({}))
        .unwrap();
    let o = &r["structuredContent"]["outputs"][0];
    assert_eq!(o["name"], "DP-1");
    assert_eq!(o["mode"]["refresh_hz"], 60.0);
    assert_eq!(o["logical"]["scale"], 2.0);
}

/// Writes an executable script without this process ever holding it open for writing.
///
/// Tests run on parallel threads that spawn processes. Had this process written the file, a
/// child forked on another thread meanwhile would inherit the write handle until its own exec,
/// and running the script in that window fails with ETXTBSY ("Text file busy"). A separate
/// `sh` writes it instead, and has exited by the time this returns.
fn write_script(path: &std::path::Path, contents: &str) {
    use std::io::Write;
    let mut sh = std::process::Command::new("sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    sh.stdin.take().unwrap().write_all(contents.as_bytes()).unwrap();
    assert!(sh.wait().unwrap().success(), "writing {}", path.display());
}

/// A pretend grim: a shell script that prints its arguments as the "image".
fn fake_grim(dir: &std::path::Path) -> String {
    let path = dir.join("grim");
    write_script(
        &path,
        "#!/bin/sh\nprintf '%s|' \"$@\"\nprintf 'WAYLAND_DISPLAY=%s' \"$WAYLAND_DISPLAY\"\n",
    );
    path.to_string_lossy().into_owned()
}

#[test]
fn screenshot_scales_to_fit_and_finds_the_display() {
    let fake = FakeHecate::start("shot", compositor);
    let config = Config {
        grim: fake_grim(&fake.dir),
        ..Config::default()
    };
    let tools = fake.tools(config);
    let r = tools.call("screenshot", &json!({ "output": "DP-1" })).unwrap();
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["content"][0]["type"], "image");
    assert_eq!(r["content"][0]["mimeType"], "image/jpeg");
    let args = decode_base64(r["content"][0]["data"].as_str().unwrap());
    // 1920 logical px into 1568: 0.8167, under the output's own scale of 2.
    assert!(args.starts_with("-o|DP-1|-s|0.8167|-t|jpeg|-q|85|-|"), "{args}");
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        assert!(
            args.ends_with("WAYLAND_DISPLAY=wayland-1"),
            "from the socket's name: {args}"
        );
    }

    let r = tools
        .call("screenshot", &json!({ "max_size": 8192, "format": "png" }))
        .unwrap();
    let args = decode_base64(r["content"][0]["data"].as_str().unwrap());
    assert!(
        args.starts_with("-s|2.0000|-t|png|-|"),
        "never past the output's scale: {args}"
    );
    assert_eq!(r["content"][0]["mimeType"], "image/png");

    let r = tools
        .call("screenshot", &json!({ "output": "HDMI-A-9" }))
        .unwrap();
    assert_eq!(r["isError"], true);
}

#[test]
fn failing_grim_is_a_tool_error() {
    let fake = FakeHecate::start("nogrim", compositor);
    let config = Config {
        grim: "/nonexistent/grim".into(),
        ..Config::default()
    };
    let r = fake.tools(config).call("screenshot", &json!({})).unwrap();
    assert_eq!(r["isError"], true);
    assert!(text(&r).contains("running /nonexistent/grim"), "{}", text(&r));
}

#[test]
fn notifications_newest_first_with_filters() {
    let fake = FakeHecate::start("notif", compositor);
    let file = fake.dir.join("notifications.json");
    std::fs::write(
        &file,
        json!([
            { "notificationId": 1, "appName": "Firefox", "summary": "Download done", "body": "a.iso",
              "time": 1_763_600_000_000_i64, "urgency": "normal", "actions": [], "appIcon": "", "image": "" },
            { "notificationId": 2, "appName": "Claude", "summary": "Waiting", "body": "Needs an answer",
              "time": 1_763_610_000_000_i64, "urgency": "critical", "actions": [{ "identifier": "open", "text": "Open" }],
              "appIcon": "", "image": "" },
        ])
        .to_string(),
    )
    .unwrap();
    let config = Config {
        notifications_file: file,
        ..Config::default()
    };
    let tools = fake.tools(config);
    let r = tools.call("list_notifications", &json!({})).unwrap();
    let list = r["structuredContent"]["notifications"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["app_name"], "Claude");
    assert_eq!(list[0]["actions"], json!(["Open"]));
    assert_eq!(list[1]["time"], "2025-11-20T00:53:20Z");
    let r = tools
        .call("list_notifications", &json!({ "query": "ISO" }))
        .unwrap();
    assert_eq!(r["structuredContent"]["total"], 1);
    let r = tools.call("list_notifications", &json!({ "limit": 1 })).unwrap();
    assert_eq!(
        r["structuredContent"]["notifications"].as_array().unwrap().len(),
        1
    );
    assert_eq!(r["structuredContent"]["total"], 2);

    let config = Config {
        notifications_file: fake.dir.join("missing.json"),
        ..Config::default()
    };
    let r = fake.tools(config).call("list_notifications", &json!({})).unwrap();
    assert_eq!(r["isError"], false);
    assert!(
        r["structuredContent"]["note"]
            .as_str()
            .unwrap()
            .contains("no notification history")
    );
}

#[test]
fn send_notification_runs_notify_send() {
    let fake = FakeHecate::start("send", compositor);
    let script = fake.dir.join("notify-send");
    let log = fake.dir.join("args");
    write_script(
        &script,
        &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\necho 42\n", log.display()),
    );
    let mut config = Config {
        notify_send: script.to_string_lossy().into_owned(),
        ..Config::default()
    };
    config.allow.insert(Permission::Notify);
    let tools = fake.tools(config);
    let r = tools
        .call(
            "send_notification",
            &json!({ "summary": "-rf", "body": "done", "urgency": "low" }),
        )
        .unwrap();
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["id"], 42);
    let args = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        args,
        "--app-name=Claude\n--urgency=low\n--print-id\n--\n-rf\ndone\n"
    );
    let r = tools
        .call("send_notification", &json!({ "summary": "x", "urgency": "loud" }))
        .unwrap();
    assert_eq!(r["isError"], true);
}

#[test]
fn helpers() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00Z");
    assert!((fit_scale(3840, 1080, 1568, 1.0) - 1568.0 / 3840.0).abs() < 1e-9);
    assert_eq!(fit_scale(800, 600, 1568, 1.0), 1.0);
}

fn decode_base64(s: &str) -> String {
    const ABC: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut n = 0;
    let mut out = Vec::new();
    for c in s.chars().filter(|c| *c != '=') {
        bits = (bits << 6) | ABC.find(c).unwrap() as u32;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
            bits &= (1 << n) - 1;
        }
    }
    String::from_utf8(out).unwrap()
}
