# Configuration

Status: proposal. Names and defaults are up for review.

## Files

Search order, first match wins:

1. `--config <path>` / `$HECATE_CONFIG`
2. `$XDG_CONFIG_HOME/hecate/config.toml`
3. `$XDG_CONFIG_HOME/hecate/config.json`
4. built-in defaults (`hecate --print-default-config` prints them as TOML)

No separate autostart, shutdown, environment or theme files: those are keys
in the config.

## Format

- **TOML**, or **JSON** with the identical shape. Both deserialise through
  serde into one typed `Config` struct. Pure data, no scripting.
- Unknown keys are an error (catches typos); missing keys take defaults.
  Validation also checks semantic errors: bad key names, unknown actions,
  colours, conflicting binds, impossible radii.
- `include = ["binds.toml", "rules.toml"]` merges other files (relative to
  the including file), later wins. Tables merge, arrays replace. Missing
  included files are skipped with a warning, so `shell.toml` can be listed
  before the shell has ever written it.
- `hecate validate <file>` validates without starting. The Nix modules run
  it at build time.
- `hecate schema` prints a JSON Schema (via `schemars`). Point taplo / Even
  Better TOML at it for completion and inline docs.
- Reload: on file change (inotify), `SIGHUP`, or `hecate msg reload`. A
  failed load keeps the running config, logs the error and runs
  `error_command` if set.

## Example: config.toml

This is roughly your niri setup translated, with the bundled shell instead of Noctalia.

```toml
# top-level keys must come before the first [table]
include = ["shell.toml"]   # optional; written by the bundled shell's settings UI, so the Nix-made file stays read-only
autostart = [            # the bundled shell starts as a user service, not from here
  ["sh", "-c", "QT_STYLE_OVERRIDE=Fusion QT_QPA_PLATFORM=xcb synology-drive autostart"],
]

[border]
width = 0                 # your niri look: no visible border...
clip_content = true       # ...but clipped rounded corners
keep_on_maximize = false

# a number for all corners, or a table per corner
[border.radius]
top_left = 15
top_right = 15
bottom_right = 15
bottom_left = 15

[border.color]
active = "#89b4fa"
inactive = "#45475a"
urgent = "#f38ba8"

[general]
decorations = "server"    # "server" (ask clients to drop CSD) | "client"
gaps = 0
error_command = ["notify-send", "hecate", "config error: {error}"]

[outputs."DP-1"]
mode = "2560x1080@200.007"
position = [3000, 180]
adaptive_sync = "fullscreen"   # "off" | "on" | "fullscreen"
focus_at_startup = true

[outputs."DP-2"]
mode = "1920x1080@60"
position = [1920, 0]
transform = "90"

[outputs."HDMI-A-1"]
mode = "1920x1080@60"
position = [0, 0]

[gaming]
allow_tearing = "fullscreen"   # "never" | "fullscreen" | "always"
direct_scanout = true
fullscreen_inhibits_idle = true

[xwayland]
enable = true
path = "xwayland-satellite"    # spawned on first X11 connection

[workspaces]
scope = "per_output"           # "global" (labwc) | "per_output" (niri/Hyprland)
kind = "fixed"                 # "fixed" | "auto" (niri-style, one empty at the end)
count = 9                      # fixed only, 1..=32
# names = ["web", "code", "chat"]   # optional, fixed only, length <= count
# min = 1                      # auto only: kept even when empty
auto_back_and_forth = true

[focus]
follow_mouse = false
raise_on_focus = true
steal = "smart"                # "never" | "smart" (needs xdg-activation token) | "always"

[placement]
policy = "center"              # "center" | "cursor" | "cascade" | "automatic"

[sizing]
width_presets = [0.3333, 0.5, 0.6667]
height_presets = [0.5, 1.0]

[snapping]
range = { inner = 10, outer = 10 }
corner_range = 50
top_maximize = true

[blur]                         # used by layer_rules and ext-background-effect
enable = true
passes = 3
radius = 6
noise = 0.02

[keyboard]
layout = "us"
repeat_rate = 25
repeat_delay = 600
numlock = true

[input."*"]
accel_profile = "flat"

[input."type:touchpad"]
tap = true
natural_scroll = true
disable_while_typing = true

[environment]
NIXOS_OZONE_WL = "1"
QT_QPA_PLATFORM = "wayland"

# binds: key combo -> action. Bare string for argument-less actions,
# inline table for actions with arguments.
[binds]
"Mod+Q" = "close"
"Mod+Return" = { spawn = ["kitty"] }
"Mod+B" = { spawn = ["zen-beta"] }
"Mod+Backspace" = { spawn = ["loginctl", "lock-session"] }
"Mod+D" = "toggle_maximize"
"Mod+F" = "toggle_fullscreen"
"Mod+R" = "cycle_width_preset"
"Mod+Left" = { focus_direction = "left" }
"Mod+Ctrl+Left" = { snap_to_edge = "left" }
"Mod+1" = { workspace = 1 }
"Mod+Shift+1" = { move_to_workspace = 1 }
"XF86AudioRaiseVolume" = { spawn = ["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", "2%+", "-l", "1.5"], allow_when_locked = true }
"Mod+Shift+Escape" = { action = "toggle_keybinds", allow_when_inhibited = true }
# press/release pair and a modifier-only tap (end-4 style)
"Super_L" = { on_press = { spawn = ["qs", "-c", "end4-pC", "ipc", "call", "shortcut", "press", "workspaceNumber"] }, on_release = { spawn = ["qs", "-c", "end4-pC", "ipc", "call", "shortcut", "release", "workspaceNumber"] }, transparent = true }
"Super" = { tap = { spawn = ["qs", "-c", "end4-pC", "ipc", "call", "shortcut", "press", "searchToggleRelease"] } }

[mouse_binds]
"Mod+BTN_LEFT" = { action = "move", context = "window" }
"Mod+BTN_RIGHT" = { action = "resize", context = "window" }

# layer rules (the shell profile adds end-4's set; these are a sample)
[[layer_rules]]
match = { namespace = "quickshell:.*" }
blur = true
blur_popups = true
ignore_alpha = 0.79
xray = true

[[layer_rules]]
match = { namespace = "quickshell:background" }
blur = false

[[window_rules]]
match = { app_id = "zen-beta", title = "Picture-in-Picture" }
always_on_top = true
omnipresent = true

[[window_rules]]
match = { app_id = "steam_app_*" }
border = { width = 0, radius = 0 }
tearing = true

[[window_rules]]
match = { app_id = "kitty" }
border = { radius = { top_left = 15, top_right = 15, bottom_right = 4, bottom_left = 4 } }
```

## Same thing from Nix (Home Manager)

```nix
wayland.windowManager.hecate = {
  shell.enable = true;   # bundled end4-pC, adds the shell keybind profile
  settings = {
    border = {
      width = 0;
      clip_content = true;
      radius = 15;
    };
    workspaces = {
      scope = "per_output";
      kind = "fixed";
      count = 9;
      auto_back_and_forth = true;
    };
    xwayland.enable = true;
    binds = {
      "Mod+Q" = "close";
      "Mod+Return".spawn = [ "kitty" ];
      "Mod+B".spawn = [ "zen-beta" ];
    } // lib.listToAttrs (lib.concatMap (n: [
      { name = "Mod+${toString n}";       value.workspace = n; }
      { name = "Mod+Shift+${toString n}"; value.move_to_workspace = n; }
    ]) (lib.range 1 9));
    window_rules = [
      { match = { app_id = "zen-beta"; title = "Picture-in-Picture"; }; always_on_top = true; }
    ];
  };
};
```

Your existing niri `binds { suffixes; prefixes; substitutions; }` helper
works unchanged once action names are mapped, because binds have the same
`"Combo".action-ish` shape.

## Notes

- **Binds**: value is either an action string, a single-key table
  (`{ workspace = 3 }`), or a table with `action` plus flags. Instead of
  `action`, a table can give `on_press` and/or `on_release` (separate
  actions for hold-style binds), or `tap` for a modifier-only bind that
  fires on release only if no other key was pressed meanwhile.
  `transparent = true` lets the key event continue to other binds and the
  client. Flags (from
  labwc): `layout_dependent`, `on_release`, `allow_when_locked`, plus
  `allow_when_inhibited` for the escape bind and `repeat` (default true for
  non-spawn actions).
- `spawn` takes an argv list (no shell); `spawn_sh` runs through `sh -c`,
  like niri's `spawn-sh`.
- `radius` accepts a number (all corners) or a table with the four named
  corners. Missing corners in a table default to 0.
- Colours accept `#rrggbb` and `#rrggbbaa`.
- **Workspaces**: `scope` and `kind` are independent. `count` and `min`
  are capped at 32 (validation error above that). Switching `kind` or
  `scope` on reload migrates windows to the nearest workspace by index.
- Output `mode` accepts `WxH`, `WxH@Hz` (closest match) or exact decimals.
- Window rule matching uses globs on `app_id`, `title`, `sandbox_engine`,
  `tag` (xdg-toplevel-tag), plus `is_x11` (via satellite). All given fields
  must match; rules apply in order, later wins.
- The full action list lives in `action.rs` and is derived from labwc's
  (see [labwc-survey.md](labwc-survey.md#actions)) plus niri-inspired
  additions (`focus_direction`, `cycle_width_preset`, `center`, ...).
