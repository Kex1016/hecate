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
    Blobs(Blobs),
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
    /// Grows each time the window gets focus: sort descending for most recently used first.
    #[serde(default)]
    pub focus_serial: u64,
    /// The `border_tint` set on it: `gold`, `love` or `foam` while it is the colour that name
    /// has in `border.tint`, else `#rrggbb`. None when its border has its usual colours.
    #[serde(default)]
    pub border_tint: Option<String>,
}

/// The `border_tint` action's argument: `{"border_tint": "gold"}`, or
/// `{"border_tint": {"color": "gold", "ms": 3000}}` to have it clear itself. The colour is
/// `gold`, `love`, `foam` (`border.tint`), any `#rrggbb`, or `none` to clear it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum BorderTint {
    Color(String),
    Timed(TimedTint),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TimedTint {
    pub color: String,
    /// Clear the tint again after this many milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
}

impl BorderTint {
    pub fn new(color: impl Into<String>, ms: Option<u64>) -> BorderTint {
        let color = color.into();
        match ms {
            None => BorderTint::Color(color),
            Some(ms) => BorderTint::Timed(TimedTint { color, ms: Some(ms) }),
        }
    }

    /// The colour, and how long it lasts.
    pub fn parts(&self) -> (&str, Option<u64>) {
        match self {
            BorderTint::Color(color) => (color, None),
            BorderTint::Timed(t) => (&t.color, t.ms),
        }
    }
}

impl<'de> Deserialize<'de> for BorderTint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Color(String),
            Timed(TimedTint),
        }
        match Raw::deserialize(d) {
            Ok(Raw::Color(c)) => Ok(BorderTint::Color(c)),
            Ok(Raw::Timed(t)) => Ok(BorderTint::Timed(t)),
            Err(_) => Err(serde::de::Error::custom(
                "border_tint takes a colour name or { color, ms }",
            )),
        }
    }
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
    /// Where the render time goes (DRM backend only): building the scene, drawing it
    /// (plane assignment and the GL render), and queueing the frame to KMS.
    #[serde(default)]
    pub avg_scene_ms: f64,
    #[serde(default)]
    pub avg_draw_ms: f64,
    #[serde(default)]
    pub avg_queue_ms: f64,
}

/// A request about the blob background (`background.blobs`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blobs {
    /// Whether it is drawn, and whether it moves on each output: a `BlobsStatus`.
    Status,
    /// Draw it or not, whatever the config file says: the runtime override
    /// `background.blobs.enable`. A shell that draws a wallpaper turns it off.
    Enable(bool),
    /// Every output's simulation as it is now: a `BlobsState`.
    Export,
    /// Continue from exported simulations: outputs that have one take it over at once, the
    /// others when they appear.
    Import(BlobsState),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobsStatus {
    pub enabled: bool,
    /// One frame that doesn't move (`background.blobs.still`).
    pub still: bool,
    /// The outputs it has been drawn on.
    pub outputs: Vec<BlobsOutputStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobsOutputStatus {
    pub output: String,
    /// The grid, in cells.
    pub width: u32,
    pub height: u32,
    /// A cell's side in physical pixels.
    pub cell: u32,
    pub blobs: usize,
    pub moving: bool,
    /// Why it doesn't move: `off`, `still`, `animations_off`, `fullscreen` or `output_off`.
    pub paused: Option<String>,
}

/// The blob simulations of a compositor, to carry over to another one (the login screen's
/// to the session's), which then draws the next frame of the same picture.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BlobsState {
    pub outputs: Vec<BlobsOutput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobsOutput {
    pub output: String,
    /// The grid the positions are on, in cells. On another grid they scale with it.
    pub width: u32,
    pub height: u32,
    /// Seconds simulated so far.
    pub time: f64,
    /// The random number generator's state: what enters next is the same too.
    pub seed: u64,
    pub blobs: Vec<Blob>,
}

/// A blob: centre, radius and velocity, in cells and cells per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub vx: f32,
    pub vy: f32,
}

/// Events on an `event_stream` connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    WorkspacesChanged {
        workspaces: Vec<Workspace>,
    },
    WindowsChanged {
        windows: Vec<Window>,
    },
    OutputsChanged {
        outputs: BTreeMap<String, Output>,
    },
    KeyboardLayoutsChanged {
        keyboard_layouts: KeyboardLayouts,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    OutputFocusChanged {
        output: Option<String>,
    },
    KeyboardLayoutSwitched {
        idx: u8,
    },
    ConfigReloaded {
        failed: bool,
    },
    /// From the `signal` action.
    Signal {
        name: String,
    },
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
        let r: Request = serde_json::from_str(r#"{"blobs":{"enable":false}}"#).unwrap();
        assert_eq!(r, Request::Blobs(Blobs::Enable(false)));
        let r: Request = serde_json::from_str(r#"{"blobs":"export"}"#).unwrap();
        assert_eq!(r, Request::Blobs(Blobs::Export));
    }

    #[test]
    fn border_tint_shapes() {
        let t: BorderTint = serde_json::from_str(r#""gold""#).unwrap();
        assert_eq!(t, BorderTint::new("gold", None));
        let t: BorderTint = serde_json::from_str(r#"{"color":"love","ms":3000}"#).unwrap();
        assert_eq!(t.parts(), ("love", Some(3000)));
        let t: BorderTint = serde_json::from_str(r#"{"color":"foam"}"#).unwrap();
        assert_eq!(t.parts(), ("foam", None));
        let err = serde_json::from_str::<BorderTint>(r#"{"colour":"gold"}"#).unwrap_err();
        assert!(err.to_string().contains("colour name or { color, ms }"), "{err}");
        assert_eq!(
            serde_json::to_string(&BorderTint::new("love", Some(5))).unwrap(),
            r#"{"color":"love","ms":5}"#
        );
    }

    #[test]
    fn window_without_border_tint_still_parses() {
        // A compositor from before `border_tint` leaves the field out.
        let mut v = serde_json::to_value(Window {
            id: 1,
            title: None,
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused: false,
            is_floating: true,
            is_urgent: false,
            layout: WindowLayout {
                window_size: (1, 1),
                tile_size: (1., 1.),
                pos_in_scrolling_layout: None,
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0., 0.),
            },
            output: None,
            geometry: Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            stacking: 0,
            is_maximized: false,
            is_fullscreen: false,
            is_minimized: false,
            always_on_top: false,
            always_on_bottom: false,
            omnipresent: false,
            is_x11: false,
            focus_serial: 0,
            border_tint: Some("gold".into()),
        })
        .unwrap();
        assert_eq!(v["border_tint"], "gold");
        v.as_object_mut().unwrap().remove("border_tint");
        let w: Window = serde_json::from_value(v).unwrap();
        assert_eq!(w.border_tint, None);
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
