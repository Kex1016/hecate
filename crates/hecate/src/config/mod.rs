//! Configuration: typed structs deserialised from TOML or JSON.
//!
//! See `docs/config.md` for the user-facing description.

pub mod action;
pub mod binds;
pub mod defaults;
pub mod keys;
pub mod load;
pub mod watch;

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
    pub splash: Splash,
    pub xwayland: Xwayland,
    pub workspaces: Workspaces,
    pub focus: Focus,
    pub placement: Placement,
    pub sizing: Sizing,
    pub snapping: Snapping,
    pub resistance: Resistance,
    pub blur: Blur,
    pub background: Background,
    pub animations: Animations,
    pub keyboard: Keyboard,
    pub cursor: Cursor,
    pub gestures: Gestures,
    /// libinput settings per device: `"*"`, `"type:touchpad"`, `"type:pointer"`, `"type:touch"`,
    /// `"type:tablet"` or a device name.
    pub input: BTreeMap<String, InputDevice>,
    pub environment: BTreeMap<String, String>,
    /// Extra variables (from hecate's environment) to import into the systemd and D-Bus
    /// activation environment at startup, besides WAYLAND_DISPLAY, DISPLAY and friends.
    pub session_variables: Vec<String>,
    pub binds: BTreeMap<String, BindValue>,
    /// Binds of the bundled shell, between the base binds and yours. Set by the Home Manager
    /// module when `shell.enable = true`; leave it to that.
    pub shell_binds: BTreeMap<String, BindValue>,
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
            splash: Splash::default(),
            xwayland: Xwayland::default(),
            workspaces: Workspaces::default(),
            focus: Focus::default(),
            placement: Placement::default(),
            sizing: Sizing::default(),
            snapping: Snapping::default(),
            resistance: Resistance::default(),
            blur: Blur::default(),
            background: Background::default(),
            animations: Animations::default(),
            keyboard: Keyboard::default(),
            cursor: Cursor::default(),
            gestures: Gestures::default(),
            input: BTreeMap::new(),
            environment: BTreeMap::new(),
            session_variables: Vec::new(),
            binds: BTreeMap::new(),
            shell_binds: BTreeMap::new(),
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

/// How a border's width is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BorderLine {
    /// One line of the whole width.
    #[default]
    Solid,
    /// Two lines with a gap between them, like CSS `double`: each line is a third of the
    /// width (5 is 2, 1, 2). `border.background` shows in the gap.
    Double,
}

/// The window's name in a notch on its top border: "app: title" in the IBM VGA 8x16 face
/// (GNU Unifont for what that lacks; a title neither can draw leaves the app name alone).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BorderLabel {
    /// On, the top border grows by a band of `border.background` under its line, so the
    /// label has room without covering the window.
    pub enable: bool,
    /// Text colour. The label's own background is `border.background`.
    pub color: Color,
    /// Where the label starts, in logical pixels from the border's inner left edge.
    pub offset: i32,
}

impl Default for BorderLabel {
    fn default() -> Self {
        BorderLabel {
            enable: false,
            color: Color::rgb(0xcd, 0xd6, 0xf4),
            offset: 15,
        }
    }
}

/// The colours `border_tint` knows by name.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BorderTints {
    /// Needs you (a session waiting for an answer).
    pub gold: Color,
    /// Critical, failed, urgent.
    pub love: Color,
    /// Done.
    pub foam: Color,
}

impl Default for BorderTints {
    fn default() -> Self {
        BorderTints {
            gold: Color::rgb(0xf6, 0xc1, 0x77),
            love: Color::rgb(0xeb, 0x6f, 0x92),
            foam: Color::rgb(0x9c, 0xcf, 0xd8),
        }
    }
}

impl BorderTints {
    /// A tint by name, or any `#rrggbb`.
    pub fn resolve(&self, name: &str) -> Option<Color> {
        match name {
            "gold" => Some(self.gold),
            "love" => Some(self.love),
            "foam" => Some(self.foam),
            other => Color::parse(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Border {
    /// Border width in logical pixels. 0 hides the border (corners can still be clipped).
    pub width: f32,
    pub style: BorderLine,
    pub radius: Radius,
    pub color: BorderColors,
    /// What fills a `double` border's gap and the label's band.
    pub background: Color,
    /// Clip window contents to the rounded shape.
    pub clip_content: bool,
    pub keep_on_maximize: bool,
    pub label: BorderLabel,
    pub tint: BorderTints,
}

impl Default for Border {
    fn default() -> Self {
        Border {
            width: 2.,
            style: BorderLine::Solid,
            radius: Radius::All(10.),
            color: BorderColors::default(),
            background: Color::rgb(0x1e, 0x1e, 0x2e),
            clip_content: true,
            keep_on_maximize: false,
            label: BorderLabel::default(),
            tint: BorderTints::default(),
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

/// Login splash: a picture of the desktop (wallpaper and shell layers, no windows), saved when
/// the session locks or ends and shown from the next session's first frame until the shell
/// runs the `splash_done` action. For a shell that does; the Home Manager module turns it on
/// with `shell.enable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Splash {
    pub enable: bool,
    /// Where the pictures are kept, as `<output>.png` (default
    /// `$XDG_STATE_HOME/hecate/snapshots`). hecate-greetd's login screen reads them from
    /// `/var/lib/hecate-greetd/users/<user>/splash`.
    pub dir: Option<String>,
    /// Save the pictures at lock and exit. Off, the splash is only shown: for a compositor
    /// that starts from another one's pictures and has nothing of its own to leave behind
    /// (hecate-greetd's login screen after a logout).
    pub save: bool,
    /// Don't put the first frame on screen while it would only be the splash and the previous
    /// compositor left the same picture there (hecate-greetd's login screen, or the session
    /// before it): the first flip waits for the shell (`splash_done`), at most 5 s, so the
    /// handover doesn't flip at all until something actually changes.
    pub hold: bool,
    /// How the splash gives way to the desktop once the shell is ready.
    pub handover: SplashHandover,
}

impl Default for Splash {
    fn default() -> Self {
        Splash {
            enable: false,
            dir: None,
            save: true,
            hold: true,
            handover: SplashHandover::Fade,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum SplashHandover {
    /// Fades out over 400 ms.
    #[default]
    Fade,
    /// Gone in one frame.
    Cut,
    /// Breaks up into growing blocks over 500 ms, then cuts. A cut when animations are off.
    Pixelate,
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

/// What the compositor draws under every output's layers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Background {
    pub blobs: Blobs,
}

/// The blob background: pixelated blobs that drift, bridge and merge, dithered into three
/// tones. Sizes and speeds are in cells. The defaults are the haiiro look.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Blobs {
    /// Draw it in place of `general.background_color`, and of the black behind a lock
    /// screen. A shell that shows a wallpaper turns it off (`hecate msg blobs off`).
    pub enable: bool,
    /// One frame that doesn't move (reduced motion). It doesn't move either while
    /// `animations.enable` is off (Game Mode) or a window is fullscreen on the output.
    pub still: bool,
    /// A cell's side in logical pixels.
    pub cell: u32,
    /// Cells per blob: a 1920x1080 output is 320x180 cells of 6, which makes 82 blobs,
    /// then `count_max` of them.
    pub density: f32,
    pub count_min: u32,
    /// 64 at most.
    pub count_max: u32,
    /// Two blobs that would make a bigger one than this pass through each other.
    pub radius_max: f32,
    /// Cells per second.
    pub speed_min: f32,
    pub speed_max: f32,
    /// How often it moves, at most.
    pub fps: u32,
    /// The three tones: the ground, the dithered halo around a blob and its core.
    pub ground: Color,
    pub halo: Color,
    pub core: Color,
    /// Write the simulations to this file at exit, for the next compositor to carry on
    /// from (`resume`): hecate-greetd's login screen hands its blobs to the session.
    pub save: Option<String>,
    /// Carry on from the simulations in this file, if it is there at startup.
    pub resume: Option<String>,
}

impl Default for Blobs {
    fn default() -> Self {
        Blobs {
            enable: false,
            still: false,
            cell: 6,
            density: 700.,
            count_min: 6,
            count_max: 60,
            radius_max: 24.,
            speed_min: 1.5,
            speed_max: 6.,
            fps: 30,
            ground: Color::rgb(0x19, 0x17, 0x24),
            halo: Color::rgb(0x26, 0x23, 0x3a),
            core: Color::rgb(0x40, 0x3d, 0x52),
            save: None,
            resume: None,
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
    /// Coarse blocks that sharpen (windows, layers, and a workspace switch's new windows).
    /// With a stepped curve the blocks halve at each step.
    Pixelate,
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
    /// Coarsest block of `pixelate`, in logical pixels: blocks shrink from this to one
    /// pixel as the animation runs.
    pub cell: u32,
}

impl Anim {
    fn new(style: AnimStyle, from: f64, ms: u32, curve: &str) -> Anim {
        Anim {
            style,
            from,
            ms,
            curve: curve.into(),
            fade: true,
            cell: 16,
        }
    }
}

/// An animation's timing: how far along it is shown at each moment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Timing {
    /// Cubic bézier `[x1, y1, x2, y2]`.
    Bezier([f64; 4]),
    /// `{ steps = 4 }`: holds, then jumps, like CSS `steps()`.
    Steps(Steps),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Steps {
    /// How many equal steps the duration is cut into.
    pub steps: u32,
    #[serde(default)]
    pub jump: StepJump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepJump {
    /// Starts at the beginning and makes its last jump as it ends (CSS `step-end`). An
    /// opening `pixelate` shows its coarsest blocks first.
    #[default]
    End,
    /// Makes its first jump right away.
    Start,
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
    /// Named timings: cubic béziers `[x1, y1, x2, y2]`, or `{ steps = 4 }`. The built-in
    /// names stay known when this table doesn't list them.
    pub curves: BTreeMap<String, Timing>,
}

impl Animations {
    /// The built-in timings.
    pub fn builtin_curves() -> BTreeMap<String, Timing> {
        let stepped = Timing::Steps(Steps {
            steps: 4,
            jump: StepJump::End,
        });
        [
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
        .map(|(k, v)| (k.to_string(), Timing::Bezier(v)))
        .chain([("stepped".to_string(), stepped)])
        .collect()
    }

    /// A timing by name: `curves` first, then the built-in ones.
    pub fn timing(&self, name: &str) -> Option<Timing> {
        self.curves
            .get(name)
            .copied()
            .or_else(|| Self::builtin_curves().get(name).copied())
    }
}

impl Default for Animations {
    fn default() -> Self {
        let curves = Self::builtin_curves();
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

/// What a libinput device is, for picking its `[input."type:…"]` section.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceKind {
    pub touchpad: bool,
    pub pointer: bool,
    pub touch: bool,
    pub tablet: bool,
}

impl Config {
    /// Settings for a device: its own section, then its type's, then `"*"` (first set wins).
    pub fn input_for(&self, name: &str, kind: DeviceKind) -> InputDevice {
        let keys = [
            Some(name),
            kind.touchpad.then_some("type:touchpad"),
            (kind.pointer && !kind.touchpad).then_some("type:pointer"),
            kind.touch.then_some("type:touch"),
            kind.tablet.then_some("type:tablet"),
            Some("*"),
        ];
        let mut merged = InputDevice::default();
        for k in keys.into_iter().flatten() {
            if let Some(c) = self.input.get(k) {
                merged.merge_from(c);
            }
        }
        merged
    }
}

/// Touchpad gestures the compositor handles itself (the rest go to clients).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Gestures {
    /// Horizontal swipes with this many fingers switch workspaces (0: off).
    pub workspace_swipe_fingers: u32,
    /// How far (touchpad units, roughly logical pixels) a swipe goes to switch.
    pub workspace_swipe_distance: f64,
    /// Swipe right for the next workspace instead of left.
    pub workspace_swipe_invert: bool,
}

impl Default for Gestures {
    fn default() -> Self {
        Gestures {
            workspace_swipe_fingers: 3,
            workspace_swipe_distance: 200.,
            workspace_swipe_invert: false,
        }
    }
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
    pub style: Option<BorderLine>,
    pub radius: Option<Radius>,
    pub color: Option<BorderColorsOverride>,
    pub clip_content: Option<bool>,
    /// Show or hide the notch label on this window.
    pub label: Option<bool>,
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
    /// `"slide"`, `"slide_left"`, `"slide_right"`, `"slide_top"`, `"slide_bottom"`, `"fade"`, `"popin"`,
    /// `"pixelate"`, `"none"`.
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
    /// Keep drawing the surface below the lock screen, like Hyprland's `session_lock_xray` but
    /// only for the layers that ask (a shell's wallpaper and clock). It gets no input.
    pub show_when_locked: Option<bool>,
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
    fn timings_and_frame_keys() {
        let c = load::from_toml_str(
            r##"
            [border]
            width = 5
            style = "double"
            background = "#1f1d2e"
            label = { enable = true, color = "#e0def4" }
            tint = { gold = "#ffcc00" }

            [animations]
            window_open = { style = "pixelate", ms = 500, curve = "chunky", cell = 32, fade = false }
            layer_open = { style = "pixelate", curve = "stepped" }
            border = { style = "none" }

            [animations.curves]
            chunky = { steps = 2, jump = "start" }
            ease = [0.2, 0.0, 0.0, 1.0]

            [splash]
            handover = "pixelate"
            "##,
        )
        .unwrap();
        assert_eq!(c.border.style, BorderLine::Double);
        assert!(c.border.label.enable);
        assert_eq!(c.border.label.offset, 15);
        assert_eq!(c.border.tint.resolve("gold"), Color::parse("#ffcc00"));
        assert_eq!(c.border.tint.resolve("love"), Color::parse("#eb6f92"));
        assert_eq!(c.border.tint.resolve("#010203"), Some(Color::rgb(1, 2, 3)));
        assert_eq!(c.border.tint.resolve("teal"), None);
        assert_eq!(
            (c.animations.window_open.style, c.animations.window_open.cell),
            (AnimStyle::Pixelate, 32)
        );
        assert_eq!(c.animations.layer_open.cell, 16);
        let chunky = Timing::Steps(Steps {
            steps: 2,
            jump: StepJump::Start,
        });
        assert_eq!(c.animations.timing("chunky"), Some(chunky));
        assert_eq!(
            c.animations.timing("ease"),
            Some(Timing::Bezier([0.2, 0.0, 0.0, 1.0]))
        );
        // Built-in names survive a `curves` table that doesn't repeat them.
        assert!(matches!(c.animations.timing("stepped"), Some(Timing::Steps(s)) if s.steps == 4));
        assert!(c.animations.timing("emphasized_decel").is_some());
        assert_eq!(c.splash.handover, SplashHandover::Pixelate);

        assert!(load::from_toml_str("[animations.curves]\nbad = { steps = 0 }").is_err());
        assert!(load::from_toml_str("[animations.curves]\nbad = { step = 2 }").is_err());
        assert!(load::from_toml_str("[animations]\nwindow_open = { curve = \"nope\" }").is_err());
        assert!(load::from_toml_str("[animations]\nwindow_open = { cell = 0 }").is_err());
        assert!(load::from_toml_str("[border]\nstyle = \"dotted\"").is_err());
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
