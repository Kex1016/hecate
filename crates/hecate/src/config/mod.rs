//! Configuration: typed structs deserialised from TOML or JSON.
//!
//! See `docs/config.md` for the user-facing description.

pub mod action;
pub mod binds;
pub mod defaults;
pub mod keys;
pub mod load;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use binds::{Bind, BindValue, MouseBind};
pub use load::{load, load_file};

/// Maximum number of workspaces, for both `fixed` and `auto` kinds.
pub const MAX_WORKSPACES: u32 = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Other config files to merge, relative to this file. Later wins; missing files are skipped.
    pub include: Vec<String>,
    /// Commands run once at startup, as argv lists.
    pub autostart: Vec<Vec<String>>,
    pub general: General,
    pub border: Border,
    pub outputs: BTreeMap<String, OutputConfig>,
    pub gaming: Gaming,
    pub xwayland: Xwayland,
    pub workspaces: Workspaces,
    pub focus: Focus,
    pub placement: Placement,
    pub sizing: Sizing,
    pub snapping: Snapping,
    pub resistance: Resistance,
    pub blur: Blur,
    pub animations: Animations,
    pub keyboard: Keyboard,
    pub cursor: Cursor,
    /// libinput settings per device: `"*"`, `"type:touchpad"`, `"type:pointer"` or a device name.
    pub input: BTreeMap<String, InputDevice>,
    pub environment: BTreeMap<String, String>,
    pub binds: BTreeMap<String, BindValue>,
    pub mouse_binds: BTreeMap<String, MouseBind>,
    pub window_rules: Vec<WindowRule>,
    pub layer_rules: Vec<LayerRule>,
    pub margins: Vec<Margin>,
    pub regions: Vec<Region>,
    /// Use the built-in base binds (labwc + niri derived). Set false to start from nothing.
    pub default_binds: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            include: Vec::new(),
            autostart: Vec::new(),
            general: General::default(),
            border: Border::default(),
            outputs: BTreeMap::new(),
            gaming: Gaming::default(),
            xwayland: Xwayland::default(),
            workspaces: Workspaces::default(),
            focus: Focus::default(),
            placement: Placement::default(),
            sizing: Sizing::default(),
            snapping: Snapping::default(),
            resistance: Resistance::default(),
            blur: Blur::default(),
            animations: Animations::default(),
            keyboard: Keyboard::default(),
            cursor: Cursor::default(),
            input: BTreeMap::new(),
            environment: BTreeMap::new(),
            binds: BTreeMap::new(),
            mouse_binds: BTreeMap::new(),
            window_rules: Vec::new(),
            layer_rules: Vec::new(),
            margins: Vec::new(),
            regions: Vec::new(),
            default_binds: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum DecorationMode {
    /// Ask clients to drop their own titlebars; hecate draws only the border.
    #[default]
    Server,
    /// Let clients draw their own decorations.
    Client,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub decorations: DecorationMode,
    /// Gap between windows and output edges when snapping or maximizing, in logical pixels.
    pub gaps: i32,
    /// Run when a config reload fails. `{error}` is replaced with the message.
    pub error_command: Option<Vec<String>>,
    /// Terminal for the default `Mod+Return` bind. Falls back to $TERMINAL, foot, kitty, alacritty.
    pub terminal: Option<Vec<String>>,
    /// Background colour behind everything.
    pub background_color: Color,
}

impl Default for General {
    fn default() -> Self {
        General {
            decorations: DecorationMode::Server,
            gaps: 0,
            error_command: None,
            terminal: None,
            background_color: Color::rgb(0x1e, 0x1e, 0x2e),
        }
    }
}

/// `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color {
            r: r as f32 / 255.,
            g: g as f32 / 255.,
            b: b as f32 / 255.,
            a: 1.,
        }
    }

    /// Premultiplied RGBA for rendering.
    pub fn premultiplied(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }

    pub fn parse(s: &str) -> Option<Color> {
        let hex = s.strip_prefix('#')?;
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        match hex.len() {
            6 => Some(Color::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => {
                let mut c = Color::rgb(byte(0)?, byte(2)?, byte(4)?);
                c.a = byte(6)? as f32 / 255.;
                Some(c)
            }
            _ => None,
        }
    }

    pub fn to_hex(self) -> String {
        let to = |v: f32| (v * 255.).round().clamp(0., 255.) as u8;
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", to(self.r), to(self.g), to(self.b))
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                to(self.r),
                to(self.g),
                to(self.b),
                to(self.a)
            )
        }
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid colour {s:?}, expected #rrggbb or #rrggbbaa"))
        })
    }
}

impl JsonSchema for Color {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Color".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$",
            "description": "#rrggbb or #rrggbbaa"
        })
    }
}

/// Corner radii, either one number for all corners or per corner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Radius {
    All(f32),
    PerCorner(Corners),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Radius {
    pub fn corners(self) -> Corners {
        match self {
            Radius::All(r) => Corners {
                top_left: r,
                top_right: r,
                bottom_right: r,
                bottom_left: r,
            },
            Radius::PerCorner(c) => c,
        }
    }
}

impl Corners {
    pub fn is_zero(&self) -> bool {
        self.top_left <= 0. && self.top_right <= 0. && self.bottom_right <= 0. && self.bottom_left <= 0.
    }

    /// Shrink every radius by `by`, clamped at zero (inner radii for content clipping).
    pub fn shrink(self, by: f32) -> Corners {
        Corners {
            top_left: (self.top_left - by).max(0.),
            top_right: (self.top_right - by).max(0.),
            bottom_right: (self.bottom_right - by).max(0.),
            bottom_left: (self.bottom_left - by).max(0.),
        }
    }

    pub fn max(&self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    /// As `[top_left, top_right, bottom_right, bottom_left]`.
    pub fn as_array(&self) -> [f32; 4] {
        [self.top_left, self.top_right, self.bottom_right, self.bottom_left]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BorderColors {
    pub active: Color,
    pub inactive: Color,
    pub urgent: Color,
}

impl Default for BorderColors {
    fn default() -> Self {
        BorderColors {
            active: Color::rgb(0x89, 0xb4, 0xfa),
            inactive: Color::rgb(0x45, 0x47, 0x5a),
            urgent: Color::rgb(0xf3, 0x8b, 0xa8),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Border {
    /// Border width in logical pixels. 0 hides the border (corners can still be clipped).
    pub width: f32,
    pub radius: Radius,
    pub color: BorderColors,
    /// Clip window contents to the rounded shape.
    pub clip_content: bool,
    pub keep_on_maximize: bool,
}

impl Default for Border {
    fn default() -> Self {
        Border {
            width: 2.,
            radius: Radius::All(10.),
            color: BorderColors::default(),
            clip_content: true,
            keep_on_maximize: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Vrr {
    #[default]
    Off,
    On,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub enum Transform {
    #[default]
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "90")]
    _90,
    #[serde(rename = "180")]
    _180,
    #[serde(rename = "270")]
    _270,
    #[serde(rename = "flipped")]
    Flipped,
    #[serde(rename = "flipped-90")]
    Flipped90,
    #[serde(rename = "flipped-180")]
    Flipped180,
    #[serde(rename = "flipped-270")]
    Flipped270,
}

impl From<Transform> for smithay::utils::Transform {
    fn from(t: Transform) -> Self {
        use smithay::utils::Transform as T;
        match t {
            Transform::Normal => T::Normal,
            Transform::_90 => T::_90,
            Transform::_180 => T::_180,
            Transform::_270 => T::_270,
            Transform::Flipped => T::Flipped,
            Transform::Flipped90 => T::Flipped90,
            Transform::Flipped180 => T::Flipped180,
            Transform::Flipped270 => T::Flipped270,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    /// Set false to disable the output.
    pub enable: Option<bool>,
    /// `WxH`, `WxH@Hz` (closest refresh) or exact decimals like `2560x1080@200.007`.
    pub mode: Option<String>,
    pub position: Option<[i32; 2]>,
    pub scale: Option<f64>,
    pub transform: Option<Transform>,
    pub adaptive_sync: Option<Vrr>,
    pub focus_at_startup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tearing {
    Never,
    #[default]
    Fullscreen,
    Always,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Gaming {
    pub allow_tearing: Tearing,
    pub direct_scanout: bool,
    pub fullscreen_inhibits_idle: bool,
}

impl Default for Gaming {
    fn default() -> Self {
        Gaming {
            allow_tearing: Tearing::Fullscreen,
            direct_scanout: true,
            fullscreen_inhibits_idle: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Xwayland {
    pub enable: bool,
    /// xwayland-satellite binary, spawned on the first X11 connection.
    pub path: String,
}

impl Default for Xwayland {
    fn default() -> Self {
        Xwayland {
            enable: true,
            path: "xwayland-satellite".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceScope {
    /// One set of workspaces shared by all outputs (labwc).
    #[default]
    Global,
    /// Each output has its own workspaces (niri, Hyprland).
    PerOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// A fixed number of workspaces (`count`).
    #[default]
    Fixed,
    /// Always one empty workspace at the end; empty ones vanish when left.
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Workspaces {
    pub scope: WorkspaceScope,
    pub kind: WorkspaceKind,
    /// Number of workspaces for `fixed`, 1..=32.
    pub count: u32,
    /// Optional names for fixed workspaces.
    pub names: Vec<String>,
    /// Workspaces kept even when empty in `auto` mode.
    pub min: u32,
    pub auto_back_and_forth: bool,
}

impl Default for Workspaces {
    fn default() -> Self {
        Workspaces {
            scope: WorkspaceScope::Global,
            kind: WorkspaceKind::Fixed,
            count: 9,
            names: Vec::new(),
            min: 1,
            auto_back_and_forth: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusSteal {
    Never,
    /// Allow when the request carries a valid xdg-activation token.
    #[default]
    Smart,
    Always,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Focus {
    pub follow_mouse: bool,
    pub follow_mouse_requires_movement: bool,
    pub raise_on_focus: bool,
    pub steal: FocusSteal,
}

impl Default for Focus {
    fn default() -> Self {
        Focus {
            follow_mouse: false,
            follow_mouse_requires_movement: true,
            raise_on_focus: true,
            steal: FocusSteal::Smart,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlacementPolicy {
    #[default]
    Center,
    Cursor,
    Cascade,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Placement {
    pub policy: PlacementPolicy,
    pub cascade_offset: [i32; 2],
}

impl Default for Placement {
    fn default() -> Self {
        Placement {
            policy: PlacementPolicy::Center,
            cascade_offset: [40, 40],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Sizing {
    /// Fractions of the usable output width cycled by `cycle_width_preset`.
    pub width_presets: Vec<f64>,
    pub height_presets: Vec<f64>,
}

impl Default for Sizing {
    fn default() -> Self {
        Sizing {
            width_presets: vec![1. / 3., 0.5, 2. / 3.],
            height_presets: vec![0.5, 1.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SnapRange {
    pub inner: i32,
    pub outer: i32,
}

impl Default for SnapRange {
    fn default() -> Self {
        SnapRange { inner: 10, outer: 10 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Snapping {
    pub range: SnapRange,
    pub corner_range: i32,
    pub top_maximize: bool,
}

impl Default for Snapping {
    fn default() -> Self {
        Snapping {
            range: SnapRange::default(),
            corner_range: 50,
            top_maximize: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Resistance {
    pub screen_edge: i32,
    pub window_edge: i32,
}

impl Default for Resistance {
    fn default() -> Self {
        Resistance {
            screen_edge: 20,
            window_edge: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Blur {
    pub enable: bool,
    pub passes: u32,
    pub radius: f32,
    pub noise: f32,
}

impl Default for Blur {
    fn default() -> Self {
        Blur {
            enable: true,
            passes: 3,
            radius: 6.,
            noise: 0.02,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnimStyle {
    #[default]
    Popin,
    Fade,
    Slide,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Anim {
    pub style: AnimStyle,
    /// Starting scale for popin.
    pub from: f64,
    pub ms: u32,
    pub curve: String,
    pub fade: bool,
}

impl Anim {
    fn new(style: AnimStyle, from: f64, ms: u32, curve: &str) -> Anim {
        Anim {
            style,
            from,
            ms,
            curve: curve.into(),
            fade: true,
        }
    }
}

impl Default for Anim {
    fn default() -> Self {
        Anim::new(AnimStyle::Popin, 0.9, 250, "emphasized_decel")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Animations {
    pub enable: bool,
    /// Global duration multiplier.
    pub slowdown: f64,
    pub window_open: Anim,
    pub window_close: Anim,
    pub window_move: Anim,
    pub border: Anim,
    pub layer_open: Anim,
    pub layer_close: Anim,
    pub workspace_switch: Anim,
    /// Named cubic béziers: `[x1, y1, x2, y2]`.
    pub curves: BTreeMap<String, [f64; 4]>,
}

impl Default for Animations {
    fn default() -> Self {
        let curves = [
            ("linear", [0.0, 0.0, 1.0, 1.0]),
            ("emphasized_decel", [0.05, 0.7, 0.1, 1.0]),
            ("emphasized_accel", [0.3, 0.0, 0.8, 0.15]),
            ("standard_decel", [0.0, 0.0, 0.0, 1.0]),
            ("menu_decel", [0.1, 1.0, 0.0, 1.0]),
            ("menu_accel", [0.52, 0.03, 0.72, 0.08]),
            ("expressive_fast_spatial", [0.42, 1.67, 0.21, 0.90]),
            ("expressive_slow_spatial", [0.39, 1.29, 0.35, 0.98]),
            ("expressive_default_spatial", [0.38, 1.21, 0.22, 1.00]),
            ("stall", [1.0, -0.1, 0.7, 0.85]),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        Animations {
            enable: true,
            slowdown: 1.0,
            window_open: Anim::new(AnimStyle::Popin, 0.8, 300, "emphasized_decel"),
            window_close: Anim::new(AnimStyle::Popin, 0.9, 200, "emphasized_decel"),
            window_move: Anim::new(AnimStyle::Slide, 1.0, 300, "emphasized_decel"),
            border: Anim::new(AnimStyle::Fade, 1.0, 1000, "emphasized_decel"),
            layer_open: Anim::new(AnimStyle::Popin, 0.93, 270, "emphasized_decel"),
            layer_close: Anim::new(AnimStyle::Popin, 0.94, 240, "menu_accel"),
            workspace_switch: Anim::new(AnimStyle::Slide, 1.0, 700, "menu_decel"),
            curves,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutScope {
    #[default]
    Global,
    Window,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Keyboard {
    pub layout: String,
    pub variant: String,
    pub model: String,
    pub options: String,
    pub rules: String,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub numlock: bool,
    pub layout_scope: LayoutScope,
}

impl Default for Keyboard {
    fn default() -> Self {
        Keyboard {
            layout: String::new(),
            variant: String::new(),
            model: String::new(),
            options: String::new(),
            rules: String::new(),
            repeat_rate: 25,
            repeat_delay: 600,
            numlock: false,
            layout_scope: LayoutScope::Global,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Cursor {
    pub theme: String,
    pub size: u32,
    /// Hide the cursor after this many milliseconds without movement.
    pub hide_after_ms: Option<u32>,
}

impl Default for Cursor {
    fn default() -> Self {
        Cursor {
            theme: "default".into(),
            size: 24,
            hide_after_ms: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccelProfile {
    Flat,
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickMethod {
    None,
    ButtonAreas,
    Clickfinger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScrollMethod {
    None,
    TwoFinger,
    Edge,
    OnButton,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct InputDevice {
    pub accel_profile: Option<AccelProfile>,
    /// -1.0 to 1.0
    pub pointer_speed: Option<f64>,
    pub natural_scroll: Option<bool>,
    pub left_handed: Option<bool>,
    pub tap: Option<bool>,
    pub tap_and_drag: Option<bool>,
    pub drag_lock: Option<bool>,
    pub middle_emulation: Option<bool>,
    pub disable_while_typing: Option<bool>,
    pub click_method: Option<ClickMethod>,
    pub scroll_method: Option<ScrollMethod>,
    pub scroll_button: Option<u32>,
    pub scroll_factor: Option<f64>,
    pub enabled: Option<bool>,
    pub calibration_matrix: Option<[f32; 6]>,
    pub map_to_output: Option<String>,
}

impl InputDevice {
    /// Fill unset fields from `other`.
    pub fn merge_from(&mut self, other: &InputDevice) {
        macro_rules! m {
            ($($f:ident),*) => { $( if self.$f.is_none() { self.$f = other.$f.clone(); } )* };
        }
        m!(
            accel_profile,
            pointer_speed,
            natural_scroll,
            left_handed,
            tap,
            tap_and_drag,
            drag_lock,
            middle_emulation,
            disable_while_typing,
            click_method,
            scroll_method,
            scroll_button,
            scroll_factor,
            enabled,
            calibration_matrix,
            map_to_output
        );
    }
}

/// Per-window border overrides from window rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct BorderOverride {
    pub width: Option<f32>,
    pub radius: Option<Radius>,
    pub color: Option<BorderColorsOverride>,
    pub clip_content: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct BorderColorsOverride {
    pub active: Option<Color>,
    pub inactive: Option<Color>,
    pub urgent: Option<Color>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct WindowMatch {
    /// Glob on app_id (X11: WM_CLASS via xwayland-satellite).
    pub app_id: Option<String>,
    /// Glob on title.
    pub title: Option<String>,
    pub sandbox_engine: Option<String>,
    pub tag: Option<String>,
    pub is_x11: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct WindowRule {
    #[serde(rename = "match")]
    pub matches: WindowMatch,
    pub border: Option<BorderOverride>,
    pub decorations: Option<DecorationMode>,
    pub always_on_top: Option<bool>,
    pub always_on_bottom: Option<bool>,
    pub omnipresent: Option<bool>,
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
    pub workspace: Option<action::WorkspaceRef>,
    pub output: Option<String>,
    pub position: Option<[i32; 2]>,
    pub size: Option<[i32; 2]>,
    pub tearing: Option<bool>,
    pub skip_taskbar: Option<bool>,
    pub ignore_focus_requests: Option<bool>,
    pub fixed_position: Option<bool>,
    pub no_animations: Option<bool>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct LayerMatch {
    /// Regex on the layer-shell namespace.
    pub namespace: Option<String>,
}

/// Animation override for a layer surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LayerAnimation {
    /// `"slide"`, `"slide_left"`, `"slide_right"`, `"slide_top"`, `"slide_bottom"`, `"fade"`, `"popin"`, `"none"`.
    Named(String),
    Popin {
        popin: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct LayerRule {
    #[serde(rename = "match")]
    pub matches: LayerMatch,
    pub blur: Option<bool>,
    pub blur_popups: Option<bool>,
    /// Only pixels with alpha above this get blur behind them.
    pub ignore_alpha: Option<f32>,
    /// Blur only the background/bottom layers (cheap, the wallpaper).
    pub xray: Option<bool>,
    pub no_keyboard_focus: Option<bool>,
    pub opacity: Option<f32>,
    pub animation: Option<LayerAnimation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Margin {
    /// Output name; unset applies to all outputs.
    pub output: Option<String>,
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub name: String,
    pub x: action::Amount,
    pub y: action::Amount,
    pub width: action::Amount,
    pub height: action::Amount,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        let c = Color::parse("#89b4fa").unwrap();
        assert_eq!(c.to_hex(), "#89b4fa");
        let c = Color::parse("#89b4fa80").unwrap();
        assert_eq!(c.to_hex(), "#89b4fa80");
        assert!(Color::parse("89b4fa").is_none());
        assert!(Color::parse("#89b4f").is_none());
    }

    #[test]
    fn radius_forms() {
        #[derive(Deserialize)]
        struct W {
            r: Radius,
        }
        let w: W = toml::from_str("r = 15").unwrap();
        assert_eq!(w.r.corners().bottom_left, 15.);
        let w: W = toml::from_str("r = { top_left = 12, bottom_right = 4 }").unwrap();
        let c = w.r.corners();
        assert_eq!(c.as_array(), [12., 0., 4., 0.]);
        assert_eq!(c.shrink(5.).as_array(), [7., 0., 0., 0.]);
    }
}
