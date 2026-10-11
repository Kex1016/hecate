use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use hecate_ipc::{Payload, Reply, Request, Selector};
use serde_json::{Value, json};

use super::fixture::Fixture;

const RED: [u8; 4] = [255, 0, 0, 255];

fn ok(reply: Option<Reply>) -> Value {
    match reply.expect("no reply") {
        Reply::Ok(p) => serde_json::to_value(p).unwrap(),
        Reply::Error(e) => panic!("IPC error: {e}"),
    }
}

fn setup() -> (Fixture, super::client::ClientId) {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.map_window(id, (400, 300), RED, Some("org.example.app"));
    (f, id)
}

#[test]
fn windows_payload_has_niri_fields() {
    let (mut f, _) = setup();
    let v = ok(f.hecate().ipc_request(Request::Windows));
    let w = &v[0];
    // What end4-pC's NiriBackend reads.
    for key in ["id", "title", "app_id", "workspace_id", "is_focused", "layout"] {
        assert!(w.get(key).is_some(), "missing {key}: {w}");
    }
    assert_eq!(w["app_id"], "org.example.app");
    assert_eq!(w["is_focused"], true);
    assert_eq!(w["layout"]["window_size"], json!([400, 300]));
}

#[test]
fn workspaces_and_outputs_payloads() {
    let (mut f, _) = setup();
    let ws = ok(f.hecate().ipc_request(Request::Workspaces));
    assert_eq!(ws.as_array().unwrap().len(), 9);
    let first = &ws[0];
    for key in ["id", "idx", "name", "output", "is_active", "is_focused"] {
        assert!(first.get(key).is_some(), "missing {key}");
    }
    assert_eq!(first["idx"], 1);
    assert_eq!(first["is_active"], true);
    assert_eq!(first["output"], "headless-1");
    assert_eq!(first["window_ids"].as_array().unwrap().len(), 1);

    let outs = ok(f.hecate().ipc_request(Request::Outputs));
    let o = &outs["headless-1"];
    assert_eq!(o["logical"]["width"], 1920);
    assert_eq!(o["logical"]["scale"], 1.0);
    assert_eq!(o["is_focused"], true);
}

#[test]
fn actions_over_ipc() {
    let (mut f, _) = setup();
    let reply = f.hecate().ipc_request(Request::Action {
        action: json!({"workspace": 3}),
        target: None,
    });
    ok(reply);
    let ws = ok(f.hecate().ipc_request(Request::Workspaces));
    assert_eq!(ws[2]["is_active"], true);

    // Move the window by selector, from another workspace.
    ok(f.hecate().ipc_request(Request::Action {
        action: json!({"move_to_workspace": 3}),
        target: Some(Selector::AppId("org.example.*".into())),
    }));
    let ws = ok(f.hecate().ipc_request(Request::Workspaces));
    assert_eq!(ws[2]["window_ids"].as_array().unwrap().len(), 1);

    let bad = f.hecate().ipc_request(Request::Action {
        action: json!("not_an_action"),
        target: None,
    });
    assert!(matches!(bad, Some(Reply::Error(_))));
}

#[test]
fn window_details_and_rule_suggestion() {
    let (mut f, _) = setup();
    let v = ok(f.hecate().ipc_request(Request::Window(Selector::Focused)));
    assert_eq!(v["window"]["app_id"], "org.example.app");
    assert_eq!(v["decoration"], "server");
    let toml = v["suggested_rule_toml"].as_str().unwrap();
    assert!(toml.contains("app_id = \"org.example.app\""));
    let none = f
        .hecate()
        .ipc_request(Request::Window(Selector::AppId("nope*".into())));
    assert!(matches!(none, Some(Reply::Error(_))));
}

#[test]
fn runtime_overrides() {
    let (mut f, _) = setup();
    ok(f.hecate().ipc_request(Request::Set {
        path: "animations.enable".into(),
        value: json!(false),
    }));
    ok(f.hecate().ipc_request(Request::Set {
        path: "border.width".into(),
        value: json!(1),
    }));
    assert!(!f.hecate().hecate.config.animations.enable);
    assert_eq!(f.hecate().hecate.config.border.width, 1.0);
    let v = ok(f.hecate().ipc_request(Request::Get {
        path: "animations.enable".into(),
    }));
    assert_eq!(v, json!(false));

    // Invalid overrides are rejected and change nothing.
    let bad = f.hecate().ipc_request(Request::Set {
        path: "workspaces.count".into(),
        value: json!(99),
    });
    assert!(matches!(bad, Some(Reply::Error(_))));
    assert_eq!(f.hecate().hecate.config.workspaces.count, 9);

    // Overrides survive a reload.
    f.hecate().reload_config();
    assert!(!f.hecate().hecate.config.animations.enable);

    ok(f.hecate().ipc_request(Request::Unset(hecate_ipc::Unset::All)));
    assert!(f.hecate().hecate.config.animations.enable);
    assert_eq!(f.hecate().hecate.config.border.width, 2.0);
}

#[test]
fn binds_and_layers() {
    let (mut f, _) = setup();
    let binds = ok(f.hecate().ipc_request(Request::Binds));
    let close = binds
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["key"] == "Mod+q")
        .expect("Mod+q bind");
    assert_eq!(close["action"], "close");
    assert_eq!(close["source"], "base");
    let layers = ok(f.hecate().ipc_request(Request::Layers));
    assert!(layers.as_array().unwrap().is_empty());
}

#[test]
fn version_and_tree() {
    let (mut f, _) = setup();
    let v = ok(f.hecate().ipc_request(Request::Version));
    assert_eq!(v["ipc"], hecate_ipc::VERSION);
    let t = ok(f.hecate().ipc_request(Request::Tree));
    assert_eq!(t[0]["name"], "headless-1");
    assert_eq!(t[0]["workspaces"][0]["windows"][0]["app_id"], "org.example.app");
}

#[test]
fn real_socket_roundtrip_and_event_stream() {
    let dir = std::env::temp_dir().join(format!("hecate-ipc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // SAFETY: tests touching the environment don't run concurrently with readers of this var.
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &dir) };
    let (mut f, id) = setup();
    crate::ipc::IpcServer::start(f.hecate()).unwrap();
    let path = f.hecate().hecate.ipc.as_ref().unwrap().path.clone();

    let mut sock = UnixStream::connect(&path).unwrap();
    sock.set_nonblocking(true).unwrap();
    let mut buf = Vec::new();
    let mut read_line = |f: &mut Fixture, sock: &mut UnixStream| -> Value {
        for _ in 0..5000 {
            f.dispatch();
            let mut chunk = [0u8; 65536];
            match sock.read(&mut chunk) {
                Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                _ => {}
            }
            if let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buf.drain(..=pos).collect();
                return serde_json::from_slice(&line).unwrap();
            }
        }
        panic!(
            "no reply on the socket (buffered: {:?})",
            String::from_utf8_lossy(&buf)
        );
    };

    sock.write_all(b"\"focused_window\"\n").unwrap();
    let v = read_line(&mut f, &mut sock);
    assert_eq!(v["ok"]["app_id"], "org.example.app");

    sock.write_all(b"\"event_stream\"\n").unwrap();
    assert_eq!(read_line(&mut f, &mut sock), json!({"ok": {}}));
    // Initial full state.
    assert!(read_line(&mut f, &mut sock).get("OutputsChanged").is_some());
    assert!(read_line(&mut f, &mut sock).get("WorkspacesChanged").is_some());
    assert!(read_line(&mut f, &mut sock).get("WindowsChanged").is_some());
    assert!(
        read_line(&mut f, &mut sock)
            .get("KeyboardLayoutsChanged")
            .is_some()
    );

    // A new window produces change events.
    f.map_window(id, (200, 200), RED, Some("second"));
    let mut saw_windows = false;
    for _ in 0..6 {
        let e = read_line(&mut f, &mut sock);
        if e.get("WindowsChanged").is_some() {
            saw_windows = true;
            break;
        }
    }
    assert!(saw_windows, "WindowsChanged after mapping a window");
    let _ = Payload::Unit(hecate_ipc::Ack {});
    std::fs::remove_dir_all(&dir).ok();
}
