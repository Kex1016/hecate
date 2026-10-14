//! Actions shared by keybinds, mouse binds and IPC.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Something hecate can do. In the config, actions without arguments are bare
/// strings (`"close"`) and actions with arguments are single-key tables
/// (`{ workspace = 3 }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    // Session
    Exit,
    Reload,
    Spawn(Vec<String>),
    SpawnSh(String),
    /// Post a named `Signal` event on the IPC event stream, for a shell listening there
    /// (cheaper than spawning a process per key press, e.g. for Alt+Tab).
    Signal(String),
    /// The shell is up: fade out the login splash (`splash.enable`).
    SplashDone,
    /// Spawn the terminal from `general.terminal`, `$TERMINAL`, or the first of foot, kitty, alacritty.
    Terminal,
    ToggleKeybinds,
    PowerOffOutputs,
    PowerOnOutputs,

    // Windows
    Close,
    Kill,
    Focus,
    Raise,
    Lower,
    Minimize,
    ToggleMaximize,
    Maximize,
    Unmaximize,
    ToggleMaximizeHorizontal,
    ToggleMaximizeVertical,
    ToggleFullscreen,
    ToggleAlwaysOnTop,
    ToggleAlwaysOnBottom,
    ToggleOmnipresent,
    ToggleBorder,
    Center,
    CycleWidthPreset,
    CycleHeightPreset,
    ResetSize,
    ResizeRelative(ResizeRelative),
    MoveRelative(Delta),
    MoveTo(Position),
    ResizeTo(Size),
    SnapToEdge(Edge),
    Unsnap,
    FocusDirection(Direction),
    FocusNext,
    FocusPrev,
    /// Interactive move (mouse binds).
    Move,
    /// Interactive resize (mouse binds).
    Resize,

    // Workspaces
    Workspace(WorkspaceRef),
    MoveToWorkspace(WorkspaceRef),
    MoveWorkspaceToOutput(OutputRef),
    ToggleShowDesktop,

    // Outputs
    FocusOutput(OutputRef),
    MoveToOutput(OutputRef),
    MoveCursor(Point),

    // Input
    SwitchKeyboardLayout(LayoutRef),
    SetCursorTheme(CursorTheme),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A workspace by 1-based index, name, or relative position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WorkspaceRef {
    Index(u32),
    Named(String),
}

impl WorkspaceRef {
    pub fn is_next(&self) -> bool {
        matches!(self, WorkspaceRef::Named(n) if n == "next")
    }
    pub fn is_prev(&self) -> bool {
        matches!(self, WorkspaceRef::Named(n) if n == "prev")
    }
}

/// An output by direction (`left`, `right`, `up`, `down`), `next`/`prev`, or connector name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct OutputRef(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LayoutRef {
    Index(u32),
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResizeRelative {
    #[serde(default)]
    pub width: Option<Amount>,
    #[serde(default)]
    pub height: Option<Amount>,
}

/// A pixel amount (`40`) or a percentage of the output (`"10%"`, `"-10%"`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Amount {
    Pixels(i32),
    Percent(#[schemars(with = "String")] Percent),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percent(pub f64);

impl Serialize for Percent {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}%", self.0))
    }
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let num = s.trim().strip_suffix('%').ok_or_else(|| {
            serde::de::Error::custom(format!("expected a percentage like \"10%\", got {s:?}"))
        })?;
        num.trim()
            .parse::<f64>()
            .map(Percent)
            .map_err(|_| serde::de::Error::custom(format!("invalid percentage {s:?}")))
    }
}

impl Amount {
    /// Resolve against a reference length (the usable output width or height).
    pub fn resolve(self, reference: i32) -> i32 {
        match self {
            Amount::Pixels(px) => px,
            Amount::Percent(Percent(p)) => (reference as f64 * p / 100.0).round() as i32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CursorTheme {
    pub name: String,
    #[serde(default = "default_cursor_size")]
    pub size: u32,
}

fn default_cursor_size() -> u32 {
    24
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Wrap {
        a: Action,
    }

    fn parse(s: &str) -> Action {
        toml::from_str::<Wrap>(s).unwrap().a
    }

    #[test]
    fn bare_and_table_actions() {
        assert_eq!(parse(r#"a = "close""#), Action::Close);
        assert_eq!(
            parse(r#"a = { workspace = 3 }"#),
            Action::Workspace(WorkspaceRef::Index(3))
        );
        assert_eq!(
            parse(r#"a = { spawn = ["foot", "-e", "htop"] }"#),
            Action::Spawn(vec!["foot".into(), "-e".into(), "htop".into()])
        );
        assert_eq!(
            parse(r#"a = { focus_direction = "left" }"#),
            Action::FocusDirection(Direction::Left)
        );
        assert_eq!(
            parse(r#"a = { resize_relative = { width = "-10%" } }"#),
            Action::ResizeRelative(ResizeRelative {
                width: Some(Amount::Percent(Percent(-10.0))),
                height: None
            })
        );
    }

    #[test]
    fn amount_resolves() {
        assert_eq!(Amount::Percent(Percent(10.0)).resolve(1920), 192);
        assert_eq!(Amount::Pixels(-40).resolve(1920), -40);
    }
}
