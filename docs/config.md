# Configuration

Status: proposal. Names and defaults are up for review.

## Files

Search order, first match wins:

1. `--config <path>` / `$WL_HEX_CONFIG`
2. `$XDG_CONFIG_HOME/wl-hex/config.toml`
3. `$XDG_CONFIG_HOME/wl-hex/config.json`
4. built-in defaults (`wl-hex --print-default-config` prints them as TOML)

No separate autostart, shutdown, environment or theme files: those are keys
in the config.

## Format

- **TOML**, or **JSON** with the identical shape. Both deserialise through
  serde into one typed `Config` struct. Pure data, no scripting.
- Unknown keys are an error (catches typos); missing keys take defaults.
  Validation also checks semantic errors: bad key names, unknown actions,
  colours, conflicting binds, impossible radii.
- `include = ["binds.toml", "rules.toml"]` merges other files (relative to
  the including file), later wins. Tables merge, arrays replace.
- `wl-hex validate <file>` validates without starting. The Nix modules run
  it at build time.
- `wl-hex schema` prints a JSON Schema (via `schemars`). Point taplo / Even
  Better TOML at it for completion and inline docs.
- Reload: on file change (inotify), `SIGHUP`, or `wl-hex msg reload`. A
  failed load keeps the running config, logs the error and runs
  `error_command` if set.

## Example: config.toml

This is roughly your niri setup translated.

```toml
# top-level keys must come before the first [table]
autostart = [
  ["noctalia-shell"],
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
error_command = ["notify-send", "wl-hex", "config error: {error}"]

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
mode = "per_output"            # "global" (labwc) | "per_output" (niri/Hyprland)
names = ["1", "2", "3", "4", "5", "6", "7", "8", "9"]
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

[blur]                         # ext-background-effect, only for surfaces that ask
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
"Mod+D" = { spawn = ["noctalia-shell", "ipc", "call", "launcher", "toggle"] }
"Mod+Backspace" = { spawn_sh = "noctalia-shell ipc call lockScreen lock" }
"Mod+F" = "toggle_maximize"
"Mod+Shift+F" = "toggle_fullscreen"
"Mod+R" = "cycle_width_preset"
"Mod+Left" = { focus_direction = "left" }
"Mod+Ctrl+Left" = { snap_to_edge = "left" }
"Mod+1" = { workspace = 1 }
"Mod+Shift+1" = { move_to_workspace = 1 }
"XF86AudioRaiseVolume" = { spawn_sh = "noctalia-shell ipc call volume increase", allow_when_locked = true }
"Mod+Shift+Escape" = { action = "toggle_keybinds", allow_when_inhibited = true }

[mouse_binds]
"Mod+BTN_LEFT" = { action = "move", context = "window" }
"Mod+BTN_RIGHT" = { action = "resize", context = "window" }

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
wayland.windowManager.wl-hex.settings = {
  border = {
    width = 0;
    clip_content = true;
    radius = 15;
  };
  workspaces = {
    mode = "per_output";
    auto_back_and_forth = true;
  };
  xwayland.enable = true;
  autostart = [ [ "noctalia-shell" ] ];
  binds = {
    "Mod+Q" = "close";
    "Mod+Return".spawn = [ "kitty" ];
    "Mod+D".spawn_sh = "noctalia-shell ipc call launcher toggle";
  } // lib.listToAttrs (lib.concatMap (n: [
    { name = "Mod+${toString n}";       value.workspace = n; }
    { name = "Mod+Shift+${toString n}"; value.move_to_workspace = n; }
  ]) (lib.range 1 9));
  window_rules = [
    { match = { app_id = "zen-beta"; title = "Picture-in-Picture"; }; always_on_top = true; }
  ];
};
```

Your existing niri `binds { suffixes; prefixes; substitutions; }` helper
works unchanged once action names are mapped, because binds have the same
`"Combo".action-ish` shape.

## Notes

- **Binds**: value is either an action string, a single-key table
  (`{ workspace = 3 }`), or a table with `action` plus flags. Flags (from
  labwc): `layout_dependent`, `on_release`, `allow_when_locked`, plus
  `allow_when_inhibited` for the escape bind and `repeat` (default true for
  non-spawn actions).
- `spawn` takes an argv list (no shell); `spawn_sh` runs through `sh -c`,
  like niri's `spawn-sh`.
- `radius` accepts a number (all corners) or a table with the four named
  corners. Missing corners in a table default to 0.
- Colours accept `#rrggbb` and `#rrggbbaa`.
- Output `mode` accepts `WxH`, `WxH@Hz` (closest match) or exact decimals.
- Window rule matching uses globs on `app_id`, `title`, `sandbox_engine`,
  `tag` (xdg-toplevel-tag), plus `is_x11` (via satellite). All given fields
  must match; rules apply in order, later wins.
- The full action list lives in `action.rs` and is derived from labwc's
  (see [labwc-survey.md](labwc-survey.md#actions)) plus niri-inspired
  additions (`focus_direction`, `cycle_width_preset`, `center`, ...).
