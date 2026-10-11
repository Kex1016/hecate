//! IPC types shared between the hecate compositor and its clients.
//!
//! Transport: a Unix stream socket at `$HECATE_SOCKET`, newline-delimited JSON. Each request
//! line gets one reply line, `{"ok": ...}` or `{"error": "..."}`. After an `event_stream`
//! request the connection carries one event per line.
//!
//! Window, workspace and output payloads use the same field names as niri's IPC where the
//! concept exists, so shells with a niri backend need few changes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// IPC protocol version. Additive changes keep the number.
pub const VERSION: u32 = 1;

/// Environment variable holding the socket path.
pub const SOCKET_ENV: &str = "HECATE_SOCKET";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Version,
    /// Run an action (same syntax as binds), optionally on a selected window.
    Action {
        action: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Selector>,
    },
    Outputs,
    FocusedOutput,
    Workspaces,
    Windows,
    FocusedWindow,
    Window(Selector),
    Rules(Selector),
    Tree,
    Layers,
    KeyboardLayouts,
    CursorPosition,
    Binds,
    Config,
    Reload,
    Validate {
        path: String,
    },
    Set {
        path: String,
        value: serde_json::Value,
    },
    Get {
        path: String,
    },
    Unset(Unset),
    Pick(PickKind),
    Identify {
        #[serde(default = "default_identify_ms")]
        ms: u32,
    },
    Clients,
    Protocols,
    Gaming,
    Frames {
        #[serde(default)]
        output: Option<String>,
    },
    LogLevel(String),
    EventStream,
    KeyStream,
}

fn default_identify_ms() -> u32 {
    1500
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unset {
    All,
    Path(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    Window,
    Layer,
    Output,
    Point,
    Color,
}

/// Which window a request is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Selector {
    Focused,
    Pick,
    Id(u64),
    AppId(String),
    Title(String),
    Pid(i32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum Reply {
    Ok(Payload),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Version(Version),
    Outputs(BTreeMap<String, Output>),
    Output(Option<Output>),
    Workspaces(Vec<Workspace>),
    Windows(Vec<Window>),
    Window(Option<Window>),
    WindowDetails(Box<WindowDetails>),
    Layers(Vec<Layer>),
    KeyboardLayouts(KeyboardLayouts),
    CursorPosition(CursorPosition),
    Binds(Vec<Bind>),
    Picked(Picked),
    Tree(Vec<TreeOutput>),
    Clients(Vec<ClientInfo>),
    Strings(Vec<String>),
    Gaming(Gaming),
    Frames(Vec<FrameStats>),
    Value(serde_json::Value),
    Unit(Ack),
}

/// Empty success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Ack {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub compositor: String,
    pub ipc: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    pub width: u16,
    pub height: u16,
    /// Millihertz.
    pub refresh_rate: u32,
    pub is_preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogicalOutput {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub transform: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: Option<String>,
    pub physical_size: Option<(u32, u32)>,
    pub modes: Vec<Mode>,
    pub current_mode: Option<usize>,
    pub vrr_supported: bool,
    pub vrr_enabled: bool,
    pub logical: Option<LogicalOutput>,
    // hecate additions
    pub is_focused: bool,
    pub usable_area: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: u64,
    /// 1-based index within its group (all outputs in global scope, one output otherwise).
    pub idx: u8,
    pub name: Option<String>,
    pub output: Option<String>,
    pub is_urgent: bool,
    pub is_active: bool,
    pub is_focused: bool,
    pub active_window_id: Option<u64>,
    // hecate additions
    pub window_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowLayout {
    pub window_size: (i32, i32),
    pub tile_size: (f64, f64),
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
    pub tile_pos_in_workspace_view: Option<(f64, f64)>,
    pub window_offset_in_tile: (f64, f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub pid: Option<i32>,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
    pub is_urgent: bool,
    pub layout: WindowLayout,
    // hecate additions
    pub output: Option<String>,
    pub geometry: Rect,
    /// Position in the workspace stack, 0 = bottom.
    pub stacking: usize,
    pub is_maximized: bool,
    pub is_fullscreen: bool,
    pub is_minimized: bool,
    pub always_on_top: bool,
    pub always_on_bottom: bool,
    pub omnipresent: bool,
    pub is_x11: bool,
}

/// Everything about one window (`hecate msg pick` / `window`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowDetails {
    pub window: Window,
    pub sandbox_engine: Option<String>,
    pub sandbox_app_id: Option<String>,
    pub decoration: String,
    pub inhibits_idle: bool,
    pub inhibits_shortcuts: bool,
    pub tearing_requested: bool,
    /// Matched window rules: (index, the rule as JSON).
    pub matched_rules: Vec<(usize, serde_json::Value)>,
    /// A rule that would match this window, as TOML.
    pub suggested_rule_toml: String,
    /// The same rule as a Nix attrset.
    pub suggested_rule_nix: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub namespace: String,
    pub layer: String,
    pub output: String,
    pub geometry: Rect,
    pub exclusive_zone: i32,
    pub keyboard_interactivity: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardLayouts {
    pub names: Vec<String>,
    pub current_idx: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bind {
    pub key: String,
    /// "base", "shell" or "config".
    pub source: String,
    pub action: serde_json::Value,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Picked {
    Window(Box<WindowDetails>),
    Layer(Layer),
    Output(String),
    Point { x: f64, y: f64, output: Option<String> },
    Color { hex: String },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeOutput {
    pub name: String,
    pub layers: Vec<Layer>,
    pub workspaces: Vec<TreeWorkspace>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeWorkspace {
    pub workspace: Workspace,
    /// Bottom to top.
    pub windows: Vec<Window>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientInfo {
    pub pid: Option<i32>,
    pub executable: Option<String>,
    pub sandboxed: bool,
    pub sandbox_engine: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gaming {
    pub outputs: Vec<GamingOutput>,
    pub windows: Vec<GamingWindow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GamingOutput {
    pub name: String,
    pub refresh_hz: f64,
    pub vrr_supported: bool,
    pub vrr_enabled: bool,
    pub tearing_allowed: bool,
    pub direct_scanout: bool,
    pub direct_scanout_blocker: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GamingWindow {
    pub id: u64,
    pub app_id: Option<String>,
    pub content_type: Option<String>,
    pub tearing_requested: bool,
    pub pointer_locked: bool,
    pub pointer_confined: bool,
    pub fullscreen: bool,
}

/// Frame timing of an output over the last few seconds of rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameStats {
    pub output: String,
    pub refresh_hz: f64,
    /// Frames presented since the output appeared.
    pub frames: u64,
    /// Vblanks missed while rendering continuously.
    pub missed: u64,
    pub avg_render_ms: f64,
    pub max_render_ms: f64,
    /// Submit to presentation.
    pub avg_latency_ms: f64,
    pub avg_interval_ms: f64,
    pub fps: f64,
}

/// Events on an `event_stream` connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    WorkspacesChanged { workspaces: Vec<Workspace> },
    WindowsChanged { windows: Vec<Window> },
    OutputsChanged { outputs: BTreeMap<String, Output> },
    KeyboardLayoutsChanged { keyboard_layouts: KeyboardLayouts },
    WindowFocusChanged { id: Option<u64> },
    WorkspaceActivated { id: u64, focused: bool },
    OutputFocusChanged { output: Option<String> },
    KeyboardLayoutSwitched { idx: u8 },
    ConfigReloaded { failed: bool },
}

/// One line on a `key_stream` connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub pressed: bool,
    pub keycode: u32,
    pub keysym: String,
    pub modifiers: Vec<String>,
    /// What to write in the config to bind this.
    pub bind: String,
    /// The bind that fired, if any.
    pub fired: Option<String>,
}

impl Reply {
    pub fn ok(payload: Payload) -> Reply {
        Reply::Ok(payload)
    }

    pub fn unit() -> Reply {
        Reply::Ok(Payload::Unit(Ack {}))
    }

    pub fn error(msg: impl Into<String>) -> Reply {
        Reply::Error(msg.into())
    }
}

/// Default socket path for a display.
pub fn socket_path(runtime_dir: &str, wayland_display: &str, pid: u32) -> String {
    format!("{runtime_dir}/hecate.{wayland_display}.{pid}.sock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shapes() {
        assert_eq!(serde_json::to_string(&Request::Windows).unwrap(), r#""windows""#);
        let r: Request = serde_json::from_str(r#"{"action":{"action":"close"}}"#).unwrap();
        assert!(matches!(r, Request::Action { target: None, .. }));
        let r: Request = serde_json::from_str(r#"{"window":{"id":4}}"#).unwrap();
        assert_eq!(r, Request::Window(Selector::Id(4)));
        let r: Request = serde_json::from_str(r#"{"pick":"window"}"#).unwrap();
        assert_eq!(r, Request::Pick(PickKind::Window));
        let r: Request = serde_json::from_str(r#"{"identify":{}}"#).unwrap();
        assert_eq!(r, Request::Identify { ms: 1500 });
    }

    #[test]
    fn reply_shapes() {
        let s = serde_json::to_string(&Reply::unit()).unwrap();
        assert_eq!(s, r#"{"ok":{}}"#);
        let s = serde_json::to_string(&Reply::error("nope")).unwrap();
        assert_eq!(s, r#"{"error":"nope"}"#);
    }

    #[test]
    fn event_shape_is_niri_like() {
        let e = Event::WindowFocusChanged { id: Some(3) };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"WindowFocusChanged":{"id":3}}"#
        );
    }
}
