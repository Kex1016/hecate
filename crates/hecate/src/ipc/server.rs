//! The control socket.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use hecate_ipc::{self as ipc, Event, Payload, Reply, Request};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, Mode, PostAction, RegistrationToken};

use crate::state::State;

/// Max bytes buffered for a slow client before it's dropped.
const MAX_OUTGOING: usize = 4 * 1024 * 1024;

pub struct IpcServer {
    pub path: PathBuf,
    next_id: u64,
    clients: Vec<IpcClient>,
    /// Last snapshot sent to event streams, to send only changes.
    last: Snapshot,
}

#[derive(Default, PartialEq)]
struct Snapshot {
    windows: String,
    workspaces: String,
    outputs: String,
    focused_window: Option<u64>,
    focused_output: Option<String>,
    layouts: String,
}

struct IpcClient {
    id: u64,
    stream: UnixStream,
    token: Option<RegistrationToken>,
    inbuf: Vec<u8>,
    outbuf: Vec<u8>,
    event_stream: bool,
    key_stream: bool,
    /// Waiting for a pick; the reply goes out when the user clicks.
    pending_pick: Option<ipc::PickKind>,
    closed: bool,
}

impl IpcServer {
    /// Bind the socket and export `HECATE_SOCKET`.
    pub fn start(state: &mut State) -> anyhow::Result<()> {
        let runtime =
            std::env::var("XDG_RUNTIME_DIR").map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR is not set"))?;
        let display = state.hecate.socket_name.clone().unwrap_or_else(|| "none".into());
        let path = PathBuf::from(ipc::socket_path(&runtime, &display, std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

        state
            .hecate
            .event_loop
            .insert_source(
                Generic::new(listener, Interest::READ, Mode::Level),
                |_, listener, state| {
                    loop {
                        match listener.accept() {
                            Ok((stream, _)) => state.ipc_accept(stream),
                            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) => {
                                tracing::warn!("IPC accept error: {e}");
                                break;
                            }
                        }
                    }
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|e| anyhow::anyhow!("inserting IPC listener: {e}"))?;

        let path_str = path.to_string_lossy().into_owned();
        crate::utils::set_child_env(ipc::SOCKET_ENV, Some(&path_str));
        // SAFETY: set during startup before other threads read the environment.
        unsafe {
            std::env::set_var(ipc::SOCKET_ENV, &path_str);
        }
        tracing::info!("IPC socket at {path_str}");
        state.hecate.ipc = Some(IpcServer {
            path,
            next_id: 1,
            clients: Vec::new(),
            last: Snapshot::default(),
        });
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl State {
    fn ipc_accept(&mut self, stream: UnixStream) {
        if let Err(err) = stream.set_nonblocking(true) {
            tracing::warn!("IPC: {err}");
            return;
        }
        let Some(server) = self.hecate.ipc.as_mut() else {
            return;
        };
        let id = server.next_id;
        server.next_id += 1;
        let Ok(read_half) = stream.try_clone() else {
            return;
        };
        let token = self
            .hecate
            .event_loop
            .insert_source(
                Generic::new(read_half, Interest::READ, Mode::Level),
                move |_, s, state| {
                    // SAFETY: we only read from the stream.
                    let stream = unsafe { s.get_mut() };
                    Ok(state.ipc_readable(id, stream))
                },
            )
            .ok();
        server.clients.push(IpcClient {
            id,
            stream,
            token,
            inbuf: Vec::new(),
            outbuf: Vec::new(),
            event_stream: false,
            key_stream: false,
            pending_pick: None,
            closed: false,
        });
    }

    fn ipc_readable(&mut self, id: u64, stream: &mut UnixStream) -> PostAction {
        let mut buf = [0u8; 4096];
        let mut lines = Vec::new();
        let mut eof = false;
        loop {
            match stream.read(&mut buf) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(n) => {
                    let Some(client) = self.ipc_client(id) else {
                        return PostAction::Remove;
                    };
                    client.inbuf.extend_from_slice(&buf[..n]);
                    while let Some(pos) = client.inbuf.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = client.inbuf.drain(..=pos).collect();
                        lines.push(line);
                    }
                    if client.inbuf.len() > MAX_OUTGOING {
                        eof = true;
                        break;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    eof = true;
                    break;
                }
            }
        }
        for line in lines {
            let text = String::from_utf8_lossy(&line);
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Request>(text) {
                Ok(req) => self.ipc_handle(id, req),
                Err(err) => Some(Reply::error(format!("invalid request: {err}"))),
            };
            if let Some(reply) = reply {
                self.ipc_send(id, &reply);
            }
        }
        if eof {
            if let Some(client) = self.ipc_client(id) {
                client.closed = true;
            }
            return PostAction::Remove;
        }
        PostAction::Continue
    }

    fn ipc_client(&mut self, id: u64) -> Option<&mut IpcClient> {
        self.hecate.ipc.as_mut()?.clients.iter_mut().find(|c| c.id == id)
    }

    fn ipc_send<T: serde::Serialize>(&mut self, id: u64, value: &T) {
        let Ok(mut line) = serde_json::to_vec(value) else {
            return;
        };
        line.push(b'\n');
        if let Some(client) = self.ipc_client(id) {
            client.outbuf.extend_from_slice(&line);
            if client.outbuf.len() > MAX_OUTGOING {
                client.closed = true;
            }
        }
        self.ipc_flush();
    }

    /// Write buffered output; drop closed clients. Called every loop iteration.
    pub fn ipc_flush(&mut self) {
        let Some(server) = self.hecate.ipc.as_mut() else {
            return;
        };
        for client in &mut server.clients {
            while !client.outbuf.is_empty() && !client.closed {
                match client.stream.write(&client.outbuf) {
                    Ok(0) => client.closed = true,
                    Ok(n) => {
                        client.outbuf.drain(..n);
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => client.closed = true,
                }
            }
        }
        let closed: Vec<Option<RegistrationToken>> = server
            .clients
            .iter_mut()
            .filter(|c| c.closed)
            .map(|c| c.token.take())
            .collect();
        server.clients.retain(|c| !c.closed);
        for t in closed.into_iter().flatten() {
            self.hecate.event_loop.remove(t);
        }
    }

    /// Handle a request without a socket (tests, internal callers).
    pub fn ipc_request(&mut self, req: Request) -> Option<Reply> {
        self.ipc_handle(0, req)
    }

    fn ipc_handle(&mut self, id: u64, req: Request) -> Option<Reply> {
        Some(match req {
            Request::Version => Reply::ok(Payload::Version(ipc::Version {
                compositor: env!("CARGO_PKG_VERSION").into(),
                ipc: ipc::VERSION,
            })),
            Request::Action { action, target } => {
                let action: crate::config::action::Action = match serde_json::from_value(action) {
                    Ok(a) => a,
                    Err(err) => return Some(Reply::error(format!("invalid action: {err}"))),
                };
                let window = match &target {
                    None => None,
                    Some(sel) => match self.select_window(sel) {
                        Ok(w) => Some(w),
                        Err(e) => return Some(Reply::error(e)),
                    },
                };
                self.do_action(action, window);
                Reply::unit()
            }
            Request::Outputs => Reply::ok(Payload::Outputs(self.ipc_outputs())),
            Request::FocusedOutput => {
                let o = self.hecate.wm.focused_output.clone();
                Reply::ok(Payload::Output(o.map(|o| self.ipc_output(&o))))
            }
            Request::Workspaces => Reply::ok(Payload::Workspaces(self.ipc_workspaces())),
            Request::Windows => Reply::ok(Payload::Windows(self.ipc_windows())),
            Request::FocusedWindow => {
                let w = self.focused_window();
                Reply::ok(Payload::Window(w.map(|w| self.ipc_window(&w))))
            }
            Request::Window(ipc::Selector::Pick) | Request::Rules(ipc::Selector::Pick) => {
                return self.ipc_start_pick(id, ipc::PickKind::Window);
            }
            Request::Window(sel) | Request::Rules(sel) => match self.select_window(&sel) {
                Ok(w) => Reply::ok(Payload::WindowDetails(Box::new(self.ipc_window_details(&w)))),
                Err(e) => Reply::error(e),
            },
            Request::Tree => Reply::ok(Payload::Tree(self.ipc_tree())),
            Request::Layers => Reply::ok(Payload::Layers(self.ipc_layers())),
            Request::KeyboardLayouts => Reply::ok(Payload::KeyboardLayouts(self.ipc_keyboard_layouts())),
            Request::CursorPosition => {
                let p = self.hecate.pointer_location;
                Reply::ok(Payload::CursorPosition(ipc::CursorPosition {
                    x: p.x,
                    y: p.y,
                    output: self.hecate.wm.output_at(p).map(|o| o.name()),
                }))
            }
            Request::Binds => Reply::ok(Payload::Binds(self.ipc_binds())),
            Request::Config => match serde_json::to_value(&self.hecate.config) {
                Ok(v) => Reply::ok(Payload::Value(v)),
                Err(e) => Reply::error(e.to_string()),
            },
            Request::Reload => {
                let before = self.hecate.config_reload_failures;
                self.reload_config();
                if self.hecate.config_reload_failures > before {
                    Reply::error(self.hecate.last_config_error.clone().unwrap_or_default())
                } else {
                    Reply::unit()
                }
            }
            Request::Validate { path } => match crate::config::load_file(std::path::Path::new(&path)) {
                Ok(_) => Reply::unit(),
                Err(e) => Reply::error(e.to_string()),
            },
            Request::Set { path, value } => match self.set_override(&path, value) {
                Ok(()) => Reply::unit(),
                Err(e) => Reply::error(e),
            },
            Request::Get { path } => match self.get_config_value(&path) {
                Some(v) => Reply::ok(Payload::Value(v)),
                None => Reply::error(format!("no config value at {path:?}")),
            },
            Request::Unset(u) => match self.unset_override(u) {
                Ok(()) => Reply::unit(),
                Err(e) => Reply::error(e),
            },
            Request::Pick(kind) => return self.ipc_start_pick(id, kind),
            Request::Identify { ms } => {
                self.identify_windows(ms);
                Reply::unit()
            }
            Request::Clients => Reply::ok(Payload::Clients(self.ipc_clients())),
            Request::Protocols => Reply::ok(Payload::Strings(crate::state::advertised_globals())),
            Request::Gaming => Reply::ok(Payload::Gaming(self.ipc_gaming())),
            Request::Frames { output } => {
                let stats: Vec<_> = self
                    .hecate
                    .output_state
                    .iter()
                    .filter(|(o, _)| output.as_ref().is_none_or(|n| *n == o.name()))
                    .map(|(o, s)| {
                        let hz = o.current_mode().map(|m| m.refresh as f64 / 1000.0).unwrap_or(0.0);
                        s.frame_stats.ipc(o.name(), hz)
                    })
                    .collect();
                if stats.is_empty() && output.is_some() {
                    Reply::error("no such output")
                } else {
                    Reply::ok(Payload::Frames(stats))
                }
            }
            Request::LogLevel(level) => {
                tracing::info!("log level change requested: {level} (restart with HECATE_LOG to persist)");
                Reply::unit()
            }
            Request::EventStream => {
                if let Some(c) = self.ipc_client(id) {
                    c.event_stream = true;
                }
                self.ipc_send(id, &Reply::unit());
                self.ipc_send_full_state(id);
                return None;
            }
            Request::KeyStream => {
                if let Some(c) = self.ipc_client(id) {
                    c.key_stream = true;
                }
                Reply::unit()
            }
        })
    }

    fn ipc_send_full_state(&mut self, id: u64) {
        let events = vec![
            Event::OutputsChanged {
                outputs: self.ipc_outputs(),
            },
            Event::WorkspacesChanged {
                workspaces: self.ipc_workspaces(),
            },
            Event::WindowsChanged {
                windows: self.ipc_windows(),
            },
            Event::KeyboardLayoutsChanged {
                keyboard_layouts: self.ipc_keyboard_layouts(),
            },
        ];
        for e in events {
            self.ipc_send(id, &e);
        }
    }

    /// Diff the state against the last snapshot and notify event streams. Called each loop.
    pub fn ipc_refresh(&mut self) {
        let has_streams = self
            .hecate
            .ipc
            .as_ref()
            .is_some_and(|s| s.clients.iter().any(|c| c.event_stream));
        if !has_streams {
            return;
        }
        let windows = self.ipc_windows();
        let workspaces = self.ipc_workspaces();
        let outputs = self.ipc_outputs();
        let layouts = self.ipc_keyboard_layouts();
        let now = Snapshot {
            windows: serde_json::to_string(&windows).unwrap_or_default(),
            workspaces: serde_json::to_string(&workspaces).unwrap_or_default(),
            outputs: serde_json::to_string(&outputs).unwrap_or_default(),
            focused_window: self.focused_window().map(|w| crate::wm::WindowExt::wid(&w)),
            focused_output: self.hecate.wm.focused_output.as_ref().map(|o| o.name()),
            layouts: serde_json::to_string(&layouts).unwrap_or_default(),
        };
        let server = self.hecate.ipc.as_mut().unwrap();
        if now == server.last {
            return;
        }
        let mut events = Vec::new();
        if now.outputs != server.last.outputs {
            events.push(Event::OutputsChanged { outputs });
        }
        if now.workspaces != server.last.workspaces {
            events.push(Event::WorkspacesChanged { workspaces });
        }
        if now.windows != server.last.windows {
            events.push(Event::WindowsChanged { windows });
        }
        if now.focused_window != server.last.focused_window {
            events.push(Event::WindowFocusChanged {
                id: now.focused_window,
            });
        }
        if now.focused_output != server.last.focused_output {
            events.push(Event::OutputFocusChanged {
                output: now.focused_output.clone(),
            });
        }
        if now.layouts != server.last.layouts {
            events.push(Event::KeyboardLayoutSwitched {
                idx: layouts.current_idx,
            });
        }
        server.last = now;
        let ids: Vec<u64> = server
            .clients
            .iter()
            .filter(|c| c.event_stream)
            .map(|c| c.id)
            .collect();
        for id in ids {
            for e in &events {
                self.ipc_send(id, e);
            }
        }
    }

    pub fn ipc_broadcast_event(&mut self, event: Event) {
        let Some(server) = self.hecate.ipc.as_ref() else {
            return;
        };
        let ids: Vec<u64> = server
            .clients
            .iter()
            .filter(|c| c.event_stream)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.ipc_send(id, &event);
        }
    }

    pub fn ipc_broadcast_key(&mut self, event: ipc::KeyEvent) {
        let Some(server) = self.hecate.ipc.as_ref() else {
            return;
        };
        let ids: Vec<u64> = server
            .clients
            .iter()
            .filter(|c| c.key_stream)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.ipc_send(id, &event);
        }
    }

    fn ipc_start_pick(&mut self, id: u64, kind: ipc::PickKind) -> Option<Reply> {
        if let Some(c) = self.ipc_client(id) {
            c.pending_pick = Some(kind);
        }
        self.hecate.cursor_status =
            smithay::input::pointer::CursorImageStatus::Named(smithay::input::pointer::CursorIcon::Crosshair);
        self.hecate.queue_redraw_all();
        None
    }

    pub fn has_pending_pick(&self) -> bool {
        self.hecate
            .ipc
            .as_ref()
            .is_some_and(|s| s.clients.iter().any(|c| c.pending_pick.is_some()))
    }

    /// Answer pending picks with what's at the pointer (or cancel).
    pub fn finish_picks(&mut self, cancelled: bool) {
        let Some(server) = self.hecate.ipc.as_mut() else {
            return;
        };
        let pending: Vec<(u64, ipc::PickKind)> = server
            .clients
            .iter_mut()
            .filter_map(|c| c.pending_pick.take().map(|k| (c.id, k)))
            .collect();
        self.hecate.cursor_status = smithay::input::pointer::CursorImageStatus::default_named();
        let pos = self.hecate.pointer_location;
        for (id, kind) in pending {
            let picked = if cancelled {
                ipc::Picked::Cancelled
            } else {
                self.pick_at(kind, pos)
            };
            self.ipc_send(id, &Reply::ok(Payload::Picked(picked)));
        }
    }

    fn pick_at(
        &mut self,
        kind: ipc::PickKind,
        pos: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) -> ipc::Picked {
        let output = self.hecate.wm.output_at(pos).map(|o| o.name());
        match kind {
            ipc::PickKind::Window => {
                let under = self.contents_under(pos);
                match under.window {
                    Some(w) => ipc::Picked::Window(Box::new(self.ipc_window_details(&w))),
                    None => ipc::Picked::Cancelled,
                }
            }
            ipc::PickKind::Layer => {
                let layers = self.ipc_layers();
                layers
                    .into_iter()
                    .rev()
                    .find(|l| {
                        let g = l.geometry;
                        pos.x >= g.x as f64
                            && pos.y >= g.y as f64
                            && pos.x < (g.x + g.width) as f64
                            && pos.y < (g.y + g.height) as f64
                    })
                    .map(ipc::Picked::Layer)
                    .unwrap_or(ipc::Picked::Cancelled)
            }
            ipc::PickKind::Output => output.map(ipc::Picked::Output).unwrap_or(ipc::Picked::Cancelled),
            ipc::PickKind::Point => ipc::Picked::Point {
                x: pos.x,
                y: pos.y,
                output,
            },
            ipc::PickKind::Color => match self.pixel_at(pos) {
                Some(hex) => ipc::Picked::Color { hex },
                None => ipc::Picked::Cancelled,
            },
        }
    }
}
