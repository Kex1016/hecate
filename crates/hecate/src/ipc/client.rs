//! `hecate msg`: talk to a running compositor.
#![allow(clippy::print_literal)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use clap::{Args, Subcommand};
use hecate_ipc::{self as ipc, PickKind, Request, Selector};
use serde_json::Value;

#[derive(Debug, Args)]
pub struct MsgArgs {
    /// Print raw JSON instead of human-readable output
    #[arg(short, long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: MsgCommand,
}

#[derive(Debug, Args, Clone, Default)]
pub struct SelectorArgs {
    /// Window id
    #[arg(long)]
    pub id: Option<u64>,
    /// Glob on app_id
    #[arg(long)]
    pub app_id: Option<String>,
    /// Glob on title
    #[arg(long)]
    pub title: Option<String>,
    /// Process id
    #[arg(long)]
    pub pid: Option<i32>,
    /// Click a window
    #[arg(long)]
    pub pick: bool,
}

impl SelectorArgs {
    fn selector(&self) -> Option<Selector> {
        if self.pick {
            Some(Selector::Pick)
        } else if let Some(id) = self.id {
            Some(Selector::Id(id))
        } else if let Some(a) = &self.app_id {
            Some(Selector::AppId(a.clone()))
        } else if let Some(t) = &self.title {
            Some(Selector::Title(t.clone()))
        } else {
            self.pid.map(Selector::Pid)
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum MsgCommand {
    /// Compositor and IPC version
    Version,
    /// Run an action: `action close`, `action workspace 3`, `action spawn foot -e htop`,
    /// or raw JSON: `action '{"snap_to_edge":"left"}'`
    Action {
        #[command(flatten)]
        target: SelectorArgs,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// List windows
    Windows,
    /// Details for one window (focused by default)
    Window {
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// Details for the focused window
    FocusedWindow,
    /// Which window rules match a window and what they set
    Rules {
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// List workspaces
    Workspaces,
    /// List outputs
    Outputs,
    /// The focused output
    FocusedOutput,
    /// Layer-shell surfaces (namespaces for layer rules)
    Layers,
    /// Outputs → layers → workspaces → windows, z-ordered
    Tree,
    /// Resolved keybinds
    Binds {
        /// Only show binds that override a base bind
        #[arg(long)]
        conflicts: bool,
    },
    /// Keyboard layouts
    KeyboardLayouts,
    /// Pointer position
    CursorPosition,
    /// Click to inspect a window (or layer/output/point/colour)
    Pick {
        /// Print a window rule matching the clicked window (TOML)
        #[arg(long)]
        rule: bool,
        /// With --rule: print it as a Nix attrset
        #[arg(long)]
        nix: bool,
        #[arg(long, conflicts_with_all = ["output", "point", "color"])]
        layer: bool,
        #[arg(long, conflicts_with_all = ["point", "color"])]
        output: bool,
        #[arg(long, conflicts_with = "color")]
        point: bool,
        #[arg(long)]
        color: bool,
    },
    /// Flash every window's border so you can match windows to the list
    Identify {
        #[arg(long, default_value_t = 1500)]
        ms: u32,
    },
    /// Live, pretty-printed event stream
    Watch,
    /// Raw JSON event stream
    EventStream,
    /// Print key presses as hecate sees them, with the bind string to use
    Keys,
    /// Reload the config file
    Reload,
    /// Validate a config file using the running compositor's parser
    Validate { path: String },
    /// Set a runtime config override, e.g. `set animations.enable false`
    Set { path: String, value: String },
    /// Get the effective config value at a path
    Get { path: String },
    /// Remove a runtime override (or all with --all)
    Unset {
        path: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// The effective config as JSON
    Config,
    /// VRR, tearing and direct scanout status per output
    Gaming,
    /// Frame timing per output: render time, presentation latency, missed frames
    Frames {
        /// Only this output
        #[arg(long)]
        output: Option<String>,
    },
    /// Connected clients
    Clients,
    /// Globals hecate advertises
    Protocols,
    /// Focus a window
    Focus {
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// Ask a window to close
    Close {
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// Kill a window's client (`kill --pick` is xkill)
    Kill {
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// Switch workspace (index, name, next, prev)
    Workspace { workspace: String },
    /// Move a window to a workspace
    MoveToWorkspace {
        workspace: String,
        #[command(flatten)]
        target: SelectorArgs,
    },
    /// Switch keyboard layout (next, prev or index)
    KeyboardLayout { layout: String },
    /// Quit the session
    Exit,
}

fn connect() -> anyhow::Result<UnixStream> {
    let path = std::env::var(ipc::SOCKET_ENV)
        .map_err(|_| anyhow::anyhow!("{} is not set; is hecate running?", ipc::SOCKET_ENV))?;
    UnixStream::connect(&path).map_err(|e| anyhow::anyhow!("connecting to {path}: {e}"))
}

fn send(stream: &mut UnixStream, req: &Request) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(req)?;
    line.push(b'\n');
    stream.write_all(&line)?;
    Ok(())
}

/// Send a request and return the `ok` value, or the error as `Err`.
fn request(req: Request) -> anyhow::Result<Value> {
    let mut stream = connect()?;
    send(&mut stream, &req)?;
    let mut reader = BufReader::new(stream);
    read_reply(&mut reader)
}

fn read_reply(reader: &mut impl BufRead) -> anyhow::Result<Value> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let v: Value =
        serde_json::from_str(&line).map_err(|e| anyhow::anyhow!("bad reply from compositor: {e}"))?;
    if let Some(err) = v.get("error") {
        anyhow::bail!("{}", err.as_str().unwrap_or("error"));
    }
    Ok(v.get("ok").cloned().unwrap_or(Value::Null))
}

/// `workspace 3` → `{"workspace": 3}`; `close` → `"close"`; several args → array.
pub fn action_from_args(args: &[String]) -> anyhow::Result<Value> {
    let first = args.first().ok_or_else(|| anyhow::anyhow!("no action given"))?;
    if first.starts_with('{') || first.starts_with('"') {
        return Ok(serde_json::from_str(&args.join(" "))?);
    }
    let name = first.replace('-', "_");
    let parse =
        |s: &str| -> Value { serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string())) };
    Ok(match &args[1..] {
        [] => Value::String(name),
        [one] if name != "spawn" => serde_json::json!({ name: parse(one) }),
        rest => {
            if name == "spawn" {
                serde_json::json!({ name: rest })
            } else if name == "spawn_sh" {
                serde_json::json!({ name: rest.join(" ") })
            } else {
                serde_json::json!({ name: rest.iter().map(|s| parse(s)).collect::<Vec<_>>() })
            }
        }
    })
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn s(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "-".into(),
        Some(other) => other.to_string(),
    }
}

fn print_windows(list: &Value) {
    let Some(arr) = list.as_array() else { return };
    println!(
        "{:<5} {:<24} {:<10} {:<5} {:<20} {:<6} {}",
        "ID", "APP_ID", "OUTPUT", "WS", "GEOMETRY", "STATE", "TITLE"
    );
    for w in arr {
        let g = &w["geometry"];
        let mut state = String::new();
        if w["is_focused"].as_bool() == Some(true) {
            state.push('*');
        }
        for (k, c) in [
            ("is_fullscreen", 'F'),
            ("is_maximized", 'M'),
            ("is_minimized", 'm'),
            ("always_on_top", 'T'),
            ("omnipresent", 'P'),
        ] {
            if w[k].as_bool() == Some(true) {
                state.push(c);
            }
        }
        println!(
            "{:<5} {:<24} {:<10} {:<5} {:<20} {:<6} {}",
            s(w, "id"),
            s(w, "app_id"),
            s(w, "output"),
            s(w, "workspace_id"),
            format!("{}x{}+{}+{}", g["width"], g["height"], g["x"], g["y"]),
            state,
            s(w, "title")
        );
    }
}

fn print_details(d: &Value) {
    let w = &d["window"];
    let g = &w["geometry"];
    println!("id:           {}", s(w, "id"));
    println!("app_id:       {}", s(w, "app_id"));
    println!("title:        {}", s(w, "title"));
    println!("pid:          {}", s(w, "pid"));
    println!("x11:          {}", s(w, "is_x11"));
    println!("output:       {}", s(w, "output"));
    println!("workspace:    {}", s(w, "workspace_id"));
    println!(
        "geometry:     {}x{} at {},{}",
        g["width"], g["height"], g["x"], g["y"]
    );
    println!("stacking:     {}", s(w, "stacking"));
    let flags: Vec<&str> = [
        ("is_focused", "focused"),
        ("is_maximized", "maximized"),
        ("is_fullscreen", "fullscreen"),
        ("is_minimized", "minimized"),
        ("always_on_top", "always-on-top"),
        ("always_on_bottom", "always-on-bottom"),
        ("omnipresent", "omnipresent"),
        ("is_urgent", "urgent"),
    ]
    .iter()
    .filter(|(k, _)| w[*k].as_bool() == Some(true))
    .map(|(_, n)| *n)
    .collect();
    println!(
        "states:       {}",
        if flags.is_empty() {
            "-".into()
        } else {
            flags.join(", ")
        }
    );
    println!("decoration:   {}", s(d, "decoration"));
    println!("sandbox:      {}", s(d, "sandbox_engine"));
    println!(
        "inhibits:     idle={} shortcuts={}",
        s(d, "inhibits_idle"),
        s(d, "inhibits_shortcuts")
    );
    println!("tearing:      {}", s(d, "tearing_requested"));
    match d["matched_rules"].as_array() {
        Some(rules) if !rules.is_empty() => {
            println!("rules:");
            for r in rules {
                println!("  [{}] {}", r[0], r[1]);
            }
        }
        _ => println!("rules:        none"),
    }
}

fn print_value_table(v: &Value) {
    match v {
        Value::Array(items) => {
            for i in items {
                println!("{}", serde_json::to_string(i).unwrap_or_default());
            }
        }
        _ => print_json(v),
    }
}

fn stream(req: Request, pretty: bool) -> anyhow::Result<()> {
    let mut stream = connect()?;
    send(&mut stream, &req)?;
    let mut reader = BufReader::new(stream);
    read_reply(&mut reader)?;
    for line in reader.lines() {
        let line = line?;
        if !pretty {
            println!("{line}");
            continue;
        }
        let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        if let Some(obj) = v.as_object()
            && let Some((name, body)) = obj.iter().next()
        {
            let summary = match name.as_str() {
                "WindowsChanged" => format!("{} windows", body["windows"].as_array().map_or(0, |a| a.len())),
                "WorkspacesChanged" => format!(
                    "{} workspaces",
                    body["workspaces"].as_array().map_or(0, |a| a.len())
                ),
                "OutputsChanged" => format!("{} outputs", body["outputs"].as_object().map_or(0, |a| a.len())),
                _ => body.to_string(),
            };
            println!("{name}: {summary}");
        } else if v.get("bind").is_some() {
            println!(
                "{} {:<28} keysym={} code={}{}",
                if v["pressed"].as_bool() == Some(true) {
                    "press  "
                } else {
                    "release"
                },
                s(&v, "bind"),
                s(&v, "keysym"),
                s(&v, "keycode"),
                match v.get("fired") {
                    Some(Value::String(f)) => format!("  → fired {f}"),
                    _ => String::new(),
                }
            );
        }
    }
    Ok(())
}

pub fn run(args: MsgArgs) -> anyhow::Result<()> {
    let json = args.json;
    let out = |v: Value, human: &dyn Fn(&Value)| {
        if json { print_json(&v) } else { human(&v) }
    };
    let target_or_focused = |t: &SelectorArgs| t.selector().unwrap_or(Selector::Focused);
    match args.command {
        MsgCommand::Version => out(request(Request::Version)?, &|v| {
            println!("hecate {} (IPC {})", s(v, "compositor"), s(v, "ipc"))
        }),
        MsgCommand::Action { target, args } => {
            let action = action_from_args(&args)?;
            request(Request::Action {
                action,
                target: target.selector(),
            })?;
        }
        MsgCommand::Windows => out(request(Request::Windows)?, &print_windows),
        MsgCommand::Window { target } | MsgCommand::Rules { target } => {
            let v = request(Request::Window(target_or_focused(&target)))?;
            let v = v
                .get("window")
                .filter(|_| v.get("decoration").is_none())
                .cloned()
                .unwrap_or(v);
            out(v, &|v| {
                if v.get("window").is_some() {
                    print_details(v)
                } else {
                    print_details(&serde_json::json!({ "window": v }))
                }
            })
        }
        MsgCommand::FocusedWindow => out(request(Request::FocusedWindow)?, &|v| {
            if v.is_null() {
                println!("no focused window")
            } else {
                print_windows(&Value::Array(vec![v.clone()]))
            }
        }),
        MsgCommand::Workspaces => out(request(Request::Workspaces)?, &|v| {
            println!(
                "{:<5} {:<4} {:<12} {:<10} {:<7} {}",
                "ID", "IDX", "NAME", "OUTPUT", "ACTIVE", "WINDOWS"
            );
            for w in v.as_array().into_iter().flatten() {
                println!(
                    "{:<5} {:<4} {:<12} {:<10} {:<7} {}",
                    s(w, "id"),
                    s(w, "idx"),
                    s(w, "name"),
                    s(w, "output"),
                    if w["is_focused"].as_bool() == Some(true) {
                        "focused"
                    } else if w["is_active"].as_bool() == Some(true) {
                        "yes"
                    } else {
                        ""
                    },
                    w["window_ids"].as_array().map_or(0, |a| a.len())
                );
            }
        }),
        MsgCommand::Outputs => out(request(Request::Outputs)?, &|v| {
            for (name, o) in v.as_object().into_iter().flatten() {
                let l = &o["logical"];
                let mode = o["current_mode"]
                    .as_u64()
                    .and_then(|i| o["modes"].get(i as usize))
                    .map(|m| {
                        format!(
                            "{}x{}@{:.3}",
                            m["width"],
                            m["height"],
                            m["refresh_rate"].as_f64().unwrap_or(0.) / 1000.
                        )
                    })
                    .unwrap_or_else(|| "-".into());
                println!(
                    "{name}{}: {} {} | {mode} | scale {} | {} | at {},{}",
                    if o["is_focused"].as_bool() == Some(true) {
                        " (focused)"
                    } else {
                        ""
                    },
                    s(o, "make"),
                    s(o, "model"),
                    l["scale"],
                    s(l, "transform"),
                    l["x"],
                    l["y"]
                );
            }
        }),
        MsgCommand::FocusedOutput => out(request(Request::FocusedOutput)?, &|v| {
            println!("{}", s(v, "name"))
        }),
        MsgCommand::Layers => out(request(Request::Layers)?, &|v| {
            println!(
                "{:<32} {:<10} {:<10} {:<8} {}",
                "NAMESPACE", "LAYER", "OUTPUT", "ZONE", "KEYBOARD"
            );
            for l in v.as_array().into_iter().flatten() {
                println!(
                    "{:<32} {:<10} {:<10} {:<8} {}",
                    s(l, "namespace"),
                    s(l, "layer"),
                    s(l, "output"),
                    s(l, "exclusive_zone"),
                    s(l, "keyboard_interactivity")
                );
            }
        }),
        MsgCommand::Tree => out(request(Request::Tree)?, &|v| {
            for o in v.as_array().into_iter().flatten() {
                println!("{}", s(o, "name"));
                for l in o["layers"].as_array().into_iter().flatten() {
                    println!("  layer {} [{}]", s(l, "namespace"), s(l, "layer"));
                }
                for ws in o["workspaces"].as_array().into_iter().flatten() {
                    let w = &ws["workspace"];
                    println!(
                        "  workspace {} {}{}",
                        s(w, "idx"),
                        s(w, "name"),
                        if w["is_active"].as_bool() == Some(true) {
                            " (active)"
                        } else {
                            ""
                        }
                    );
                    for win in ws["windows"].as_array().into_iter().flatten().rev() {
                        println!(
                            "    [{}] {} — {}",
                            s(win, "id"),
                            s(win, "app_id"),
                            s(win, "title")
                        );
                    }
                }
            }
        }),
        MsgCommand::Binds { conflicts } => out(request(Request::Binds)?, &|v| {
            for b in v.as_array().into_iter().flatten() {
                if conflicts && s(b, "source") == "base" {
                    continue;
                }
                println!("{:<28} {:<7} {}", s(b, "key"), s(b, "source"), b["action"]);
            }
        }),
        MsgCommand::KeyboardLayouts => out(request(Request::KeyboardLayouts)?, &|v| {
            let cur = v["current_idx"].as_u64().unwrap_or(0) as usize;
            for (i, n) in v["names"].as_array().into_iter().flatten().enumerate() {
                println!(
                    "{} {i}: {}",
                    if i == cur { "*" } else { " " },
                    n.as_str().unwrap_or("")
                );
            }
        }),
        MsgCommand::CursorPosition => out(request(Request::CursorPosition)?, &|v| {
            println!("{}, {} on {}", v["x"], v["y"], s(v, "output"))
        }),
        MsgCommand::Pick {
            rule,
            nix,
            layer,
            output,
            point,
            color,
        } => {
            let kind = if layer {
                PickKind::Layer
            } else if output {
                PickKind::Output
            } else if point {
                PickKind::Point
            } else if color {
                PickKind::Color
            } else {
                PickKind::Window
            };
            eprintln!("click to pick (Esc or right click cancels)…");
            let v = request(Request::Pick(kind))?;
            if json {
                print_json(&v);
            } else if let Some(w) = v.get("window") {
                if rule {
                    print!(
                        "{}",
                        s(
                            w,
                            if nix {
                                "suggested_rule_nix"
                            } else {
                                "suggested_rule_toml"
                            }
                        )
                    );
                } else {
                    print_details(w);
                }
            } else if v.get("cancelled").is_some() || v == "cancelled" {
                eprintln!("cancelled");
            } else if let Some(c) = v.get("color") {
                println!("{}", s(c, "hex"));
            } else if let Some(o) = v.get("output") {
                println!("{}", o.as_str().unwrap_or(""));
            } else {
                print_value_table(&v);
            }
        }
        MsgCommand::Identify { ms } => {
            request(Request::Identify { ms })?;
        }
        MsgCommand::Watch => stream(Request::EventStream, true)?,
        MsgCommand::EventStream => stream(Request::EventStream, false)?,
        MsgCommand::Keys => {
            eprintln!("press keys (Ctrl+C to stop)…");
            stream(Request::KeyStream, !json)?
        }
        MsgCommand::Reload => {
            request(Request::Reload)?;
        }
        MsgCommand::Validate { path } => {
            let abs = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone().into());
            request(Request::Validate {
                path: abs.to_string_lossy().into_owned(),
            })?;
            println!("{path}: ok");
        }
        MsgCommand::Set { path, value } => {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            request(Request::Set { path, value })?;
        }
        MsgCommand::Get { path } => out(request(Request::Get { path })?, &|v| match v {
            Value::String(s) => println!("{s}"),
            other => println!("{other}"),
        }),
        MsgCommand::Unset { path, all } => {
            let which = match (all, path) {
                (true, _) => ipc::Unset::All,
                (false, Some(p)) => ipc::Unset::Path(p),
                (false, None) => anyhow::bail!("give a path or --all"),
            };
            request(Request::Unset(which))?;
        }
        MsgCommand::Config => print_json(&request(Request::Config)?),
        MsgCommand::Gaming => out(request(Request::Gaming)?, &|v| {
            for o in v["outputs"].as_array().into_iter().flatten() {
                println!(
                    "{}: {:.2} Hz | VRR {} | tearing {} | direct scanout {}",
                    s(o, "name"),
                    o["refresh_hz"].as_f64().unwrap_or(0.),
                    if o["vrr_enabled"].as_bool() == Some(true) {
                        "on"
                    } else {
                        "off"
                    },
                    if o["tearing_allowed"].as_bool() == Some(true) {
                        "allowed"
                    } else {
                        "off"
                    },
                    match o.get("direct_scanout_blocker") {
                        Some(Value::String(b)) => format!("no ({b})"),
                        _ => "yes".into(),
                    }
                );
            }
        }),
        MsgCommand::Frames { output } => out(request(Request::Frames { output })?, &|v| {
            for o in v.as_array().into_iter().flatten() {
                let f = |k: &str| o[k].as_f64().unwrap_or(0.);
                println!(
                    "{}: {:.1} fps of {:.2} Hz | render {:.2} ms (max {:.2}; scene {:.2}, draw {:.2}, queue {:.2}) | latency {:.2} ms | {} frames, {} missed",
                    s(o, "output"),
                    f("fps"),
                    f("refresh_hz"),
                    f("avg_render_ms"),
                    f("max_render_ms"),
                    f("avg_scene_ms"),
                    f("avg_draw_ms"),
                    f("avg_queue_ms"),
                    f("avg_latency_ms"),
                    s(o, "frames"),
                    s(o, "missed"),
                );
            }
        }),
        MsgCommand::Clients => out(request(Request::Clients)?, &|v| {
            for c in v.as_array().into_iter().flatten() {
                println!(
                    "{:<8} {:<40} {}",
                    s(c, "pid"),
                    s(c, "executable"),
                    if c["sandboxed"].as_bool() == Some(true) {
                        format!("sandboxed ({})", s(c, "sandbox_engine"))
                    } else {
                        String::new()
                    }
                );
            }
        }),
        MsgCommand::Protocols => out(request(Request::Protocols)?, &|v| {
            for p in v.as_array().into_iter().flatten() {
                println!("{}", p.as_str().unwrap_or(""));
            }
        }),
        MsgCommand::Focus { target } => action_on("focus", &target)?,
        MsgCommand::Close { target } => action_on("close", &target)?,
        MsgCommand::Kill { target } => action_on("kill", &target)?,
        MsgCommand::Workspace { workspace } => {
            request(Request::Action {
                action: action_from_args(&["workspace".into(), workspace])?,
                target: None,
            })?;
        }
        MsgCommand::MoveToWorkspace { workspace, target } => {
            request(Request::Action {
                action: action_from_args(&["move_to_workspace".into(), workspace])?,
                target: target.selector(),
            })?;
        }
        MsgCommand::KeyboardLayout { layout } => {
            request(Request::Action {
                action: action_from_args(&["switch_keyboard_layout".into(), layout])?,
                target: None,
            })?;
        }
        MsgCommand::Exit => {
            request(Request::Action {
                action: Value::String("exit".into()),
                target: None,
            })?;
        }
    }
    Ok(())
}

fn action_on(name: &str, target: &SelectorArgs) -> anyhow::Result<()> {
    let target = match target.selector() {
        Some(Selector::Pick) => {
            eprintln!("click a window…");
            let v = request(Request::Pick(PickKind::Window))?;
            let id = v["window"]["window"]["id"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("cancelled"))?;
            Some(Selector::Id(id))
        }
        other => other,
    };
    request(Request::Action {
        action: Value::String(name.into()),
        target,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(args: &[&str]) -> Value {
        action_from_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn actions_from_cli_args() {
        assert_eq!(a(&["close"]), serde_json::json!("close"));
        assert_eq!(a(&["workspace", "3"]), serde_json::json!({"workspace": 3}));
        assert_eq!(
            a(&["workspace", "next"]),
            serde_json::json!({"workspace": "next"})
        );
        assert_eq!(
            a(&["spawn", "foot", "-e", "htop"]),
            serde_json::json!({"spawn": ["foot", "-e", "htop"]})
        );
        assert_eq!(a(&["spawn", "foot"]), serde_json::json!({"spawn": ["foot"]}));
        assert_eq!(
            a(&["snap-to-edge", "left"]),
            serde_json::json!({"snap_to_edge": "left"})
        );
        assert_eq!(
            a(&[r#"{"resize_relative":{"width":"10%"}}"#]),
            serde_json::json!({"resize_relative": {"width": "10%"}})
        );
        // Every produced value parses as a real Action.
        for v in [
            a(&["close"]),
            a(&["workspace", "3"]),
            a(&["spawn", "foot"]),
            a(&["snap-to-edge", "left"]),
        ] {
            serde_json::from_value::<crate::config::action::Action>(v).unwrap();
        }
    }
}
