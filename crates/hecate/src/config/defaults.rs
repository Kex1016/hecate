//! Built-in base binds (docs/keybinds.md, "base" layer).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use super::{BindValue, MouseBind};

pub const BASE_BINDS_TOML: &str = r#"
[binds]
"Mod+Q" = "close"
"Alt+F4" = "close"
"Mod+Shift+Alt+Q" = "kill"

"Mod+Left" = { focus_direction = "left" }
"Mod+Right" = { focus_direction = "right" }
"Mod+Up" = { focus_direction = "up" }
"Mod+Down" = { focus_direction = "down" }
"Mod+J" = { focus_direction = "left" }
"Mod+K" = { focus_direction = "down" }
"Mod+L" = { focus_direction = "up" }
"Mod+M" = { focus_direction = "right" }

"Mod+Ctrl+Left" = { snap_to_edge = "left" }
"Mod+Ctrl+Right" = { snap_to_edge = "right" }
"Mod+Ctrl+Up" = "toggle_maximize"
"Mod+Ctrl+Down" = "unsnap"

"Mod+Shift+Left" = { focus_output = "left" }
"Mod+Shift+Right" = { focus_output = "right" }
"Mod+Shift+Up" = { focus_output = "up" }
"Mod+Shift+Down" = { focus_output = "down" }
"Mod+Shift+Ctrl+Left" = { move_to_output = "left" }
"Mod+Shift+Ctrl+Right" = { move_to_output = "right" }
"Mod+Shift+Ctrl+Up" = { move_to_output = "up" }
"Mod+Shift+Ctrl+Down" = { move_to_output = "down" }

"Mod+F" = "toggle_maximize"
"Mod+Shift+F" = "toggle_fullscreen"
"Mod+Ctrl+F" = "toggle_maximize_horizontal"
"Mod+C" = "center"
"Mod+R" = "cycle_width_preset"
"Mod+Semicolon" = "cycle_height_preset"
"Mod+Apostrophe" = "reset_size"
"Mod+KP_5" = { resize_relative = { width = "-10%" } }
"Mod+KP_6" = { resize_relative = { width = "10%" } }
"Mod+Shift+KP_5" = { resize_relative = { height = "-10%" } }
"Mod+Shift+KP_6" = { resize_relative = { height = "10%" } }
"Mod+P" = "toggle_omnipresent"
"Mod+Alt+Space" = "toggle_always_on_top"
"Alt+Tab" = "focus_next"
"Alt+Shift+Tab" = "focus_prev"
"Mod+Shift+Escape" = { action = "toggle_keybinds", allow_when_inhibited = true }

"Mod+1" = { workspace = 1 }
"Mod+2" = { workspace = 2 }
"Mod+3" = { workspace = 3 }
"Mod+4" = { workspace = 4 }
"Mod+5" = { workspace = 5 }
"Mod+6" = { workspace = 6 }
"Mod+7" = { workspace = 7 }
"Mod+8" = { workspace = 8 }
"Mod+9" = { workspace = 9 }
"Mod+0" = { workspace = 10 }
"Mod+Shift+1" = { move_to_workspace = 1 }
"Mod+Shift+2" = { move_to_workspace = 2 }
"Mod+Shift+3" = { move_to_workspace = 3 }
"Mod+Shift+4" = { move_to_workspace = 4 }
"Mod+Shift+5" = { move_to_workspace = 5 }
"Mod+Shift+6" = { move_to_workspace = 6 }
"Mod+Shift+7" = { move_to_workspace = 7 }
"Mod+Shift+8" = { move_to_workspace = 8 }
"Mod+Shift+9" = { move_to_workspace = 9 }
"Mod+Shift+0" = { move_to_workspace = 10 }
"Mod+U" = { workspace = "next" }
"Mod+Page_Down" = { workspace = "next" }
"Mod+WheelScrollDown" = { workspace = "next" }
"Mod+I" = { workspace = "prev" }
"Mod+Page_Up" = { workspace = "prev" }
"Mod+WheelScrollUp" = { workspace = "prev" }
"Mod+Ctrl+U" = { move_to_workspace = "next" }
"Mod+Ctrl+Page_Down" = { move_to_workspace = "next" }
"Mod+Ctrl+I" = { move_to_workspace = "prev" }
"Mod+Ctrl+Page_Up" = { move_to_workspace = "prev" }
"Mod+Shift+U" = { move_workspace_to_output = "next" }
"Mod+Shift+I" = { move_workspace_to_output = "prev" }

"Mod+Return" = "terminal"
"Mod+Shift+E" = "exit"
"Mod+Ctrl+Alt+R" = "reload"
"Mod+Shift+L" = { spawn = ["systemctl", "suspend"] }

"XF86AudioRaiseVolume" = { spawn = ["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "5%+", "-l", "1.5"], allow_when_locked = true }
"XF86AudioLowerVolume" = { spawn = ["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"], allow_when_locked = true }
"XF86AudioMute" = { spawn = ["wpctl", "set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"], allow_when_locked = true }
"XF86AudioMicMute" = { spawn = ["wpctl", "set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"], allow_when_locked = true }
"XF86MonBrightnessUp" = { spawn = ["brightnessctl", "set", "+10%"], allow_when_locked = true }
"XF86MonBrightnessDown" = { spawn = ["brightnessctl", "set", "10%-"], allow_when_locked = true }
"XF86AudioPlay" = { spawn = ["playerctl", "play-pause"], allow_when_locked = true }
"XF86AudioPause" = { spawn = ["playerctl", "play-pause"], allow_when_locked = true }
"XF86AudioNext" = { spawn = ["playerctl", "next"], allow_when_locked = true }
"XF86AudioPrev" = { spawn = ["playerctl", "previous"], allow_when_locked = true }

[mouse_binds]
"Mod+BTN_LEFT" = { action = "move", context = "window" }
"Mod+BTN_MIDDLE" = { action = "move", context = "window" }
"Mod+BTN_RIGHT" = { action = "resize", context = "window" }
"BTN_LEFT" = { action = "resize", context = "border" }
"#;

#[derive(serde::Deserialize)]
struct BaseBinds {
    binds: BTreeMap<String, BindValue>,
    mouse_binds: BTreeMap<String, MouseBind>,
}

static BASE: LazyLock<BaseBinds> =
    LazyLock::new(|| toml::from_str(BASE_BINDS_TOML).expect("built-in binds must parse"));

pub fn base_binds() -> &'static BTreeMap<String, BindValue> {
    &BASE.binds
}

pub fn base_mouse_binds() -> &'static BTreeMap<String, MouseBind> {
    &BASE.mouse_binds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::keys::KeyCombo;

    #[test]
    fn base_binds_parse() {
        assert!(base_binds().len() > 50);
        for k in base_binds().keys().chain(base_mouse_binds().keys()) {
            k.parse::<KeyCombo>().unwrap_or_else(|e| panic!("{k}: {e}"));
        }
    }
}
