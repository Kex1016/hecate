//! The tools: what each takes, what it asks hecate, and what it hands back.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use hecate_ipc::{self as ipc, Request, Selector};
use serde_json::{Map, Value, json};

use crate::config::{Config, Permission};
use crate::hecate::Hecate;

/// Longest edge of a screenshot by default: about what a vision model looks at anyway.
const DEFAULT_MAX_SIZE: u32 = 1568;
const GRIM_TIMEOUT: Duration = Duration::from_secs(10);

/// What a tool needs before it may run.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Needs {
    Nothing,
    Screenshots,
    Allowed(Permission),
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    needs: Needs,
    /// Properties of the input object, and which are required.
    properties: fn() -> Value,
    required: &'static [&'static str],
    run: fn(&Tools, &Value) -> Result<Output, String>,
}

enum Output {
    Json(Value),
    Image {
        data: Vec<u8>,
        mime: &'static str,
        note: String,
    },
}

pub struct Tools {
    config: Config,
    hecate: Hecate,
}

fn no_properties() -> Value {
    json!({})
}

fn selector_properties() -> Value {
    json!({
        "id": { "type": "integer", "description": "Window id, from list_windows" },
        "app_id": { "type": "string", "description": "Glob on the app id, e.g. \"kitty\" or \"org.gnome.*\"" },
        "title": { "type": "string", "description": "Glob on the title, e.g. \"*README*\"" },
        "pid": { "type": "integer", "description": "Process id of the window's client" },
    })
}

fn with_selector(extra: Value) -> Value {
    let mut p = selector_properties();
    if let (Some(p), Value::Object(extra)) = (p.as_object_mut(), extra) {
        p.extend(extra);
    }
    p
}

fn workspace_property() -> Value {
    json!({
        "type": ["integer", "string"],
        "description": "1-based workspace index (idx in list_workspaces), a workspace name, \"next\" or \"prev\"",
    })
}

const TOOLS: &[Tool] = &[
    Tool {
        name: "list_windows",
        title: "List windows",
        description: "Every window hecate manages: id, app id, title, workspace, output, geometry (logical \
                      px), state flags, border tint, and focus_serial (higher = used more recently). \
                      Optional filters narrow the list.",
        needs: Needs::Nothing,
        properties: || {
            json!({
                "app_id": { "type": "string", "description": "Only windows whose app id contains this (case-insensitive)" },
                "title": { "type": "string", "description": "Only windows whose title contains this (case-insensitive)" },
                "workspace_id": { "type": "integer", "description": "Only windows on this workspace id" },
                "output": { "type": "string", "description": "Only windows on this output, e.g. \"DP-1\"" },
            })
        },
        required: &[],
        run: Tools::list_windows,
    },
    Tool {
        name: "get_window",
        title: "Window details",
        description: "Everything about one window: its state, sandbox, decoration, idle and shortcut \
                      inhibitors, the window rules that match it and a rule that would. Pick it by id, app \
                      id, title or pid; with none of them, the focused window.",
        needs: Needs::Nothing,
        properties: selector_properties,
        required: &[],
        run: Tools::get_window,
    },
    Tool {
        name: "list_workspaces",
        title: "List workspaces",
        description: "Every workspace: id, idx (1-based index), name, output, whether it is active, focused \
                      or urgent, and the ids of its windows.",
        needs: Needs::Nothing,
        properties: no_properties,
        required: &[],
        run: Tools::list_workspaces,
    },
    Tool {
        name: "list_outputs",
        title: "List outputs",
        description: "Every monitor: connector name, make and model, current mode, logical position, size \
                      and scale, VRR, and which one has the focus.",
        needs: Needs::Nothing,
        properties: no_properties,
        required: &[],
        run: Tools::list_outputs,
    },
    Tool {
        name: "list_notifications",
        title: "List notifications",
        description: "Desktop notifications the hecate shell keeps in its history, newest first: app, \
                      summary, body, urgency and time.",
        needs: Needs::Nothing,
        properties: || {
            json!({
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "How many, newest first (default 20)" },
                "app_name": { "type": "string", "description": "Only from apps whose name contains this (case-insensitive)" },
                "query": { "type": "string", "description": "Only those whose summary or body contains this (case-insensitive)" },
            })
        },
        required: &[],
        run: Tools::list_notifications,
    },
    Tool {
        name: "screenshot",
        title: "Screenshot",
        description: "A picture of the screen: every output, one output, or the area of one window (what is \
                      on screen there, so a window covered by another shows the other). Scaled down so the \
                      longest edge is at most max_size px.",
        needs: Needs::Screenshots,
        properties: || {
            json!({
                "output": { "type": "string", "description": "Only this output, e.g. \"DP-1\"" },
                "window_id": { "type": "integer", "description": "Only the area of this window (it must be on a shown workspace)" },
                "max_size": { "type": "integer", "minimum": 64, "maximum": 8192, "description": "Longest edge in px (default 1568)" },
                "format": { "type": "string", "enum": ["jpeg", "png"], "description": "Default jpeg" },
            })
        },
        required: &[],
        run: Tools::screenshot,
    },
    Tool {
        name: "focus_window",
        title: "Focus a window",
        description: "Give a window the keyboard focus and raise it. Pick it by id, app id, title or pid.",
        needs: Needs::Allowed(Permission::Windows),
        properties: selector_properties,
        required: &[],
        run: Tools::focus_window,
    },
    Tool {
        name: "move_window_to_workspace",
        title: "Move a window to a workspace",
        description: "Move a window (by id, app id, title or pid; the focused one with none) to another \
                      workspace.",
        needs: Needs::Allowed(Permission::Windows),
        properties: || with_selector(json!({ "workspace": workspace_property() })),
        required: &["workspace"],
        run: Tools::move_window_to_workspace,
    },
    Tool {
        name: "switch_workspace",
        title: "Switch workspace",
        description: "Show another workspace on the focused output.",
        needs: Needs::Allowed(Permission::Windows),
        properties: || json!({ "workspace": workspace_property() }),
        required: &["workspace"],
        run: Tools::switch_workspace,
    },
    Tool {
        name: "set_border_tint",
        title: "Tint a window's border",
        description: "Colour a window's border until cleared or for ms milliseconds, over its focus and \
                      urgency colours: gold = needs you, love = critical or failed, foam = done, any \
                      #rrggbb, or none to clear it. Picks the window by id, app id, title or pid; the \
                      focused one with none. list_windows reads tints back.",
        needs: Needs::Allowed(Permission::Tint),
        properties: || {
            with_selector(json!({
                "color": { "type": "string", "description": "gold, love, foam, a #rrggbb colour, or none" },
                "ms": { "type": "integer", "minimum": 1, "description": "Clear it again after this many milliseconds" },
            }))
        },
        required: &["color"],
        run: Tools::set_border_tint,
    },
    Tool {
        name: "send_notification",
        title: "Send a notification",
        description: "Show a desktop notification (through notify-send, so the hecate shell shows it like \
                      any other).",
        needs: Needs::Allowed(Permission::Notify),
        properties: || {
            json!({
                "summary": { "type": "string", "description": "The title line" },
                "body": { "type": "string", "description": "The text under it" },
                "urgency": { "type": "string", "enum": ["low", "normal", "critical"], "description": "Default normal" },
                "app_name": { "type": "string", "description": "Shown as the sender (default \"Claude\")" },
                "timeout_ms": { "type": "integer", "minimum": 0, "description": "How long it stays up; 0 = until dismissed" },
            })
        },
        required: &["summary"],
        run: Tools::send_notification,
    },
];

impl Tools {
    pub fn new(config: Config, hecate: Hecate) -> Tools {
        Tools { config, hecate }
    }

    fn available(&self, tool: &Tool) -> bool {
        match tool.needs {
            Needs::Nothing => true,
            Needs::Screenshots => self.config.screenshots,
            Needs::Allowed(p) => self.config.allows(p),
        }
    }

    /// The `tools/list` entries: only what this server may run.
    pub fn list(&self) -> Vec<Value> {
        TOOLS
            .iter()
            .filter(|t| self.available(t))
            .map(|t| {
                let read_only = !matches!(t.needs, Needs::Allowed(_));
                json!({
                    "name": t.name,
                    "title": t.title,
                    "description": t.description,
                    "inputSchema": {
                        "type": "object",
                        "properties": (t.properties)(),
                        "required": t.required,
                        "additionalProperties": false,
                    },
                    "annotations": {
                        "title": t.title,
                        "readOnlyHint": read_only,
                        "destructiveHint": false,
                        "idempotentHint": t.name != "send_notification",
                        "openWorldHint": false,
                    },
                })
            })
            .collect()
    }

    /// What the server tells the client about itself.
    pub fn instructions(&self) -> String {
        let allowed: Vec<&str> = self.config.allow.iter().map(|p| p.name()).collect();
        let mut s = String::from(
            "Tools for hecate, the Wayland compositor this desktop runs: read windows, workspaces, \
             outputs and the shell's notifications, and take screenshots.",
        );
        if allowed.is_empty() {
            s.push_str(
                " It is read-only: changing anything (focus, workspaces, border tints, notifications) \
                 needs hecate-mcp to be started with --allow, or `allow` in ~/.config/hecate/mcp.toml.",
            );
        } else {
            s.push_str(&format!(" Allowed to change: {}.", allowed.join(", ")));
        }
        if !self.config.screenshots {
            s.push_str(" Screenshots are turned off.");
        }
        s
    }

    /// Run a tool: None when there is no such tool, else a `tools/call` result.
    pub fn call(&self, name: &str, args: &Value) -> Option<Value> {
        let tool = TOOLS.iter().find(|t| t.name == name)?;
        let outcome = if !self.available(tool) {
            Err(match tool.needs {
                Needs::Allowed(p) => format!(
                    "{name} is not allowed: start hecate-mcp with --allow {0}, or add \"{0}\" to `allow` in \
                     ~/.config/hecate/mcp.toml",
                    p.name()
                ),
                _ => "screenshots are turned off (screenshots = false in ~/.config/hecate/mcp.toml)".into(),
            })
        } else if !args.is_object() && !args.is_null() {
            Err("arguments must be an object".into())
        } else {
            check_args(tool, args).and_then(|()| (tool.run)(self, args))
        };
        Some(match outcome {
            Ok(Output::Json(v)) => {
                let structured = match v {
                    Value::Object(_) => v,
                    other => json!({ "result": other }),
                };
                json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&structured).unwrap_or_default() }],
                    "structuredContent": structured,
                    "isError": false,
                })
            }
            Ok(Output::Image { data, mime, note }) => json!({
                "content": [
                    { "type": "image", "data": base64(&data), "mimeType": mime },
                    { "type": "text", "text": note },
                ],
                "isError": false,
            }),
            Err(e) => json!({
                "content": [{ "type": "text", "text": e }],
                "isError": true,
            }),
        })
    }

    // Read-only tools.

    fn list_windows(&self, args: &Value) -> Result<Output, String> {
        let windows: Vec<ipc::Window> = self.hecate.request(&Request::Windows)?;
        let contains = |hay: &Option<String>, key: &str| match str_arg(args, key) {
            None => true,
            Some(needle) => hay
                .as_deref()
                .is_some_and(|h| h.to_lowercase().contains(&needle.to_lowercase())),
        };
        let ws = args.get("workspace_id").and_then(Value::as_u64);
        let output = str_arg(args, "output");
        let list: Vec<Value> = windows
            .iter()
            .filter(|w| contains(&w.app_id, "app_id") && contains(&w.title, "title"))
            .filter(|w| ws.is_none() || w.workspace_id == ws)
            .filter(|w| output.is_none() || w.output.as_deref() == output)
            .map(window_summary)
            .collect();
        Ok(Output::Json(json!({ "windows": list })))
    }

    fn get_window(&self, args: &Value) -> Result<Output, String> {
        let sel = selector(args)?.unwrap_or(Selector::Focused);
        let details = self.hecate.request_value(&Request::Window(sel))?;
        Ok(Output::Json(details))
    }

    fn list_workspaces(&self, _: &Value) -> Result<Output, String> {
        let ws = self.hecate.request_value(&Request::Workspaces)?;
        Ok(Output::Json(json!({ "workspaces": ws })))
    }

    fn list_outputs(&self, _: &Value) -> Result<Output, String> {
        let outputs: std::collections::BTreeMap<String, ipc::Output> =
            self.hecate.request(&Request::Outputs)?;
        let list: Vec<Value> = outputs
            .values()
            .map(|o| {
                let mode = o.current_mode.and_then(|i| o.modes.get(i));
                json!({
                    "name": o.name,
                    "make": o.make,
                    "model": o.model,
                    "is_focused": o.is_focused,
                    "mode": mode.map(|m| json!({
                        "width": m.width,
                        "height": m.height,
                        "refresh_hz": m.refresh_rate as f64 / 1000.0,
                    })),
                    "logical": o.logical,
                    "vrr_enabled": o.vrr_enabled,
                })
            })
            .collect();
        Ok(Output::Json(json!({ "outputs": list })))
    }

    fn list_notifications(&self, args: &Value) -> Result<Output, String> {
        let path = &self.config.notifications_file;
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .clamp(1, 200) as usize;
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Output::Json(json!({
                    "notifications": [],
                    "total": 0,
                    "note": format!("no notification history at {} (is the hecate shell running?)", path.display()),
                })));
            }
            Err(e) => return Err(format!("reading {}: {e}", path.display())),
        };
        let all = parse_notifications(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let lower = |k| str_arg(args, k).map(str::to_lowercase);
        let (app, query) = (lower("app_name"), lower("query"));
        let matching: Vec<&Notification> = all
            .iter()
            .filter(|n| app.as_ref().is_none_or(|a| n.app_name.to_lowercase().contains(a)))
            .filter(|n| {
                query
                    .as_ref()
                    .is_none_or(|q| n.summary.to_lowercase().contains(q) || n.body.to_lowercase().contains(q))
            })
            .collect();
        let list: Vec<Value> = matching.iter().take(limit).map(|n| n.to_json()).collect();
        Ok(Output::Json(
            json!({ "notifications": list, "total": matching.len() }),
        ))
    }

    fn screenshot(&self, args: &Value) -> Result<Output, String> {
        let outputs: std::collections::BTreeMap<String, ipc::Output> =
            self.hecate.request(&Request::Outputs)?;
        let logical: Vec<(&str, &ipc::LogicalOutput)> = outputs
            .values()
            .filter_map(|o| o.logical.as_ref().map(|l| (o.name.as_str(), l)))
            .collect();
        if logical.is_empty() {
            return Err("no output is on".into());
        }
        let max_scale = logical.iter().map(|(_, l)| l.scale).fold(1.0, f64::max);
        let max_size = args
            .get("max_size")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_SIZE as u64)
            .clamp(64, 8192) as u32;
        let png = str_arg(args, "format") == Some("png");

        let mut grim_args: Vec<String> = Vec::new();
        let (what, width, height) = if let Some(id) = args.get("window_id").and_then(Value::as_u64) {
            let details: ipc::WindowDetails = self.hecate.request(&Request::Window(Selector::Id(id)))?;
            let w = details.window;
            if w.is_minimized {
                return Err(format!("window {id} is minimized: nothing of it is on screen"));
            }
            let workspaces: Vec<ipc::Workspace> = self.hecate.request(&Request::Workspaces)?;
            let shown = w.omnipresent
                || workspaces
                    .iter()
                    .any(|ws| Some(ws.id) == w.workspace_id && ws.is_active);
            if !shown {
                return Err(format!(
                    "window {id} is on a workspace that is not shown; focus it or switch to it first"
                ));
            }
            let g = w.geometry;
            grim_args.extend(["-g".into(), format!("{},{} {}x{}", g.x, g.y, g.width, g.height)]);
            let name = w.app_id.or(w.title).unwrap_or_default();
            (format!("window {id} ({name})"), g.width, g.height)
        } else if let Some(name) = str_arg(args, "output") {
            let (_, l) = logical
                .iter()
                .find(|(n, _)| *n == name)
                .ok_or_else(|| format!("no output {name:?} (list_outputs names them)"))?;
            grim_args.extend(["-o".into(), name.to_string()]);
            (format!("output {name}"), l.width as i32, l.height as i32)
        } else {
            let x0 = logical.iter().map(|(_, l)| l.x).min().unwrap_or(0);
            let y0 = logical.iter().map(|(_, l)| l.y).min().unwrap_or(0);
            let x1 = logical
                .iter()
                .map(|(_, l)| l.x + l.width as i32)
                .max()
                .unwrap_or(0);
            let y1 = logical
                .iter()
                .map(|(_, l)| l.y + l.height as i32)
                .max()
                .unwrap_or(0);
            ("every output".into(), x1 - x0, y1 - y0)
        };
        if width <= 0 || height <= 0 {
            return Err(format!("{what} has no area"));
        }
        let scale = fit_scale(width as u32, height as u32, max_size, max_scale);
        grim_args.extend(["-s".into(), format!("{scale:.4}")]);
        if png {
            grim_args.extend(["-t".into(), "png".into()]);
        } else {
            grim_args.extend(["-t".into(), "jpeg".into(), "-q".into(), "85".into()]);
        }
        grim_args.push("-".into());
        let data = self.run_grim(&grim_args)?;
        let (w, h) = (
            (width as f64 * scale).round() as u32,
            (height as f64 * scale).round() as u32,
        );
        Ok(Output::Image {
            data,
            mime: if png { "image/png" } else { "image/jpeg" },
            note: format!(
                "Screenshot of {what}: {width}x{height} logical px, shown at about {w}x{h} (scale {scale:.3})."
            ),
        })
    }

    fn run_grim(&self, args: &[String]) -> Result<Vec<u8>, String> {
        let mut cmd = Command::new(&self.config.grim);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(display) = self.hecate.wayland_display() {
            cmd.env("WAYLAND_DISPLAY", display);
        }
        let out =
            run_with_timeout(cmd, GRIM_TIMEOUT).map_err(|e| format!("running {}: {e}", self.config.grim))?;
        if !out.status.success() {
            return Err(format!(
                "{} failed ({}): {}",
                self.config.grim,
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if out.stdout.is_empty() {
            return Err(format!("{} wrote no image", self.config.grim));
        }
        Ok(out.stdout)
    }

    // Tools that change something.

    fn focus_window(&self, args: &Value) -> Result<Output, String> {
        let sel = selector(args)?.ok_or("say which window: id, app_id, title or pid")?;
        self.hecate.action(json!("focus"), Some(sel))?;
        Ok(Output::Json(json!({ "focused": true })))
    }

    fn move_window_to_workspace(&self, args: &Value) -> Result<Output, String> {
        let ws = workspace_arg(args)?;
        let sel = selector(args)?;
        self.hecate.action(json!({ "move_to_workspace": ws }), sel)?;
        Ok(Output::Json(json!({ "moved_to": ws })))
    }

    fn switch_workspace(&self, args: &Value) -> Result<Output, String> {
        let ws = workspace_arg(args)?;
        self.hecate.action(json!({ "workspace": ws }), None)?;
        Ok(Output::Json(json!({ "workspace": ws })))
    }

    fn set_border_tint(&self, args: &Value) -> Result<Output, String> {
        let color = str_arg(args, "color").ok_or("color is required")?;
        let ms = args.get("ms").and_then(Value::as_u64);
        let tint = ipc::BorderTint::new(color, ms);
        let sel = selector(args)?;
        let action = json!({ "border_tint": tint });
        self.hecate.action(action, sel)?;
        Ok(Output::Json(json!({ "border_tint": color, "ms": ms })))
    }

    fn send_notification(&self, args: &Value) -> Result<Output, String> {
        let summary = str_arg(args, "summary").ok_or("summary is required")?;
        let mut cmd = Command::new(&self.config.notify_send);
        cmd.arg(format!(
            "--app-name={}",
            str_arg(args, "app_name").unwrap_or("Claude")
        ));
        if let Some(u) = str_arg(args, "urgency") {
            if !["low", "normal", "critical"].contains(&u) {
                return Err(format!("urgency is low, normal or critical, not {u:?}"));
            }
            cmd.arg(format!("--urgency={u}"));
        }
        if let Some(t) = args.get("timeout_ms").and_then(Value::as_u64) {
            cmd.arg(format!("--expire-time={t}"));
        }
        cmd.arg("--print-id").arg("--").arg(summary);
        if let Some(body) = str_arg(args, "body") {
            cmd.arg(body);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let out = run_with_timeout(cmd, Duration::from_secs(10))
            .map_err(|e| format!("running {}: {e}", self.config.notify_send))?;
        if !out.status.success() {
            return Err(format!(
                "{} failed: {}",
                self.config.notify_send,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let id = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok();
        Ok(Output::Json(json!({ "sent": true, "id": id })))
    }
}

/// Refuse unknown arguments and missing required ones, so a typo is not silently ignored.
fn check_args(tool: &Tool, args: &Value) -> Result<(), String> {
    let empty = Map::new();
    let given = args.as_object().unwrap_or(&empty);
    let props = (tool.properties)();
    for key in given.keys() {
        if props.get(key).is_none() {
            return Err(format!("{} takes no argument {key:?}", tool.name));
        }
    }
    for key in tool.required {
        if given.get(*key).is_none_or(Value::is_null) {
            return Err(format!("{} needs {key:?}", tool.name));
        }
    }
    Ok(())
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// At most one of id, app_id, title, pid.
fn selector(args: &Value) -> Result<Option<Selector>, String> {
    let mut found = Vec::new();
    if let Some(v) = args.get("id").filter(|v| !v.is_null()) {
        found.push(Selector::Id(v.as_u64().ok_or("id is a window id (a number)")?));
    }
    if let Some(s) = str_arg(args, "app_id") {
        found.push(Selector::AppId(s.into()));
    }
    if let Some(s) = str_arg(args, "title") {
        found.push(Selector::Title(s.into()));
    }
    if let Some(v) = args.get("pid").filter(|v| !v.is_null()) {
        let pid = v
            .as_i64()
            .and_then(|p| i32::try_from(p).ok())
            .ok_or("pid is a number")?;
        found.push(Selector::Pid(pid));
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        _ => Err("pick the window by one of id, app_id, title or pid, not several".into()),
    }
}

fn workspace_arg(args: &Value) -> Result<Value, String> {
    match args.get("workspace") {
        Some(Value::Number(n)) if n.as_u64().is_some_and(|n| n >= 1) => Ok(Value::Number(n.clone())),
        Some(Value::String(s)) if !s.is_empty() => {
            // "3" means the third, as on the command line.
            Ok(s.parse::<u64>()
                .map(Value::from)
                .unwrap_or_else(|_| Value::String(s.clone())))
        }
        _ => Err("workspace is a 1-based index, a name, \"next\" or \"prev\"".into()),
    }
}

fn window_summary(w: &ipc::Window) -> Value {
    json!({
        "id": w.id,
        "app_id": w.app_id,
        "title": w.title,
        "pid": w.pid,
        "workspace_id": w.workspace_id,
        "output": w.output,
        "geometry": w.geometry,
        "is_focused": w.is_focused,
        "is_urgent": w.is_urgent,
        "is_minimized": w.is_minimized,
        "is_maximized": w.is_maximized,
        "is_fullscreen": w.is_fullscreen,
        "always_on_top": w.always_on_top,
        "omnipresent": w.omnipresent,
        "border_tint": w.border_tint,
        "focus_serial": w.focus_serial,
    })
}

/// The grim scale that fits `w`×`h` logical px into `max` px, never above the outputs' own.
fn fit_scale(w: u32, h: u32, max: u32, max_output_scale: f64) -> f64 {
    let long = w.max(h).max(1) as f64;
    (max as f64 / long).min(max_output_scale)
}

/// One entry of the shell's `notifications.json` (`services/Notifications.qml`).
#[derive(Debug, PartialEq)]
struct Notification {
    id: Option<u64>,
    app_name: String,
    summary: String,
    body: String,
    urgency: Value,
    /// Milliseconds since the epoch.
    time: Option<i64>,
    actions: Vec<String>,
}

impl Notification {
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "app_name": self.app_name,
            "summary": self.summary,
            "body": self.body,
            "urgency": self.urgency,
            "time": self.time.map(rfc3339),
            "actions": self.actions,
        })
    }
}

/// Newest first.
fn parse_notifications(text: &str) -> Result<Vec<Notification>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let arr = v.as_array().ok_or("not a list of notifications")?;
    let s = |n: &Value, k: &str| n.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut list: Vec<Notification> = arr
        .iter()
        .map(|n| Notification {
            id: n.get("notificationId").and_then(Value::as_u64),
            app_name: s(n, "appName"),
            summary: s(n, "summary"),
            body: s(n, "body"),
            urgency: n.get("urgency").cloned().unwrap_or(Value::Null),
            time: n.get("time").and_then(Value::as_f64).map(|t| t as i64),
            actions: n
                .get("actions")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|a| a.get("text").and_then(Value::as_str).map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    list.sort_by_key(|n| std::cmp::Reverse(n.time));
    Ok(list)
}

/// Milliseconds since the epoch as `YYYY-MM-DDTHH:MM:SSZ`.
fn rfc3339(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<std::process::Output> {
    let mut child = cmd.spawn()?;
    // Read both pipes on threads so a big image can't fill one and stall the child.
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = pipe(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err = pipe(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(std::process::Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

fn base64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
