# Configuration

Status: proposal. Names and defaults are up for review.

## Files

Search order, first match wins:

1. `--config <path>` / `$WL_HEX_CONFIG`
2. `$XDG_CONFIG_HOME/wl-hex/init.lua`
3. `$XDG_CONFIG_HOME/wl-hex/config.json`
4. built-in defaults (also printed by `wl-hex --print-default-config`)

No separate autostart, shutdown, environment or theme files: those are keys
in the config.

## Format

**Lua** evaluates to a table. The state is sandboxed: no `os.execute`,
`io`, `require` outside the config dir, no network. Allowed helpers:
`os.getenv`, `wl_hex.hostname()`, string/table/math libs.

The table goes through serde into the typed `Config` struct, so
**`config.json`** with the same shape is equally valid. Unknown keys are an
error (catches typos); missing keys take defaults.

Reload: on file change (inotify) or `SIGHUP`. A failed parse or validation
keeps the running config, logs the error and runs `error_command` if set.

The config is data, not code: Lua functions in the table are rejected in v1.
If we later want event hooks (e.g. `on_window_open`), they'd be an explicit
opt-in section, not a change to this model.

## Schema sketch

```lua
-- ~/.config/wl-hex/init.lua
local mod = "Super"

local binds = {
  { key = mod .. "+Return",       action = { spawn = { "foot" } } },
  { key = mod .. "+q",            action = "close" },
  { key = mod .. "+f",            action = "toggle_fullscreen" },
  { key = mod .. "+a",            action = "toggle_maximize" },
  { key = mod .. "+Shift+e",      action = "exit" },
  { key = mod .. "+Shift+r",      action = "reload" },
  { key = mod .. "+Left",         action = { snap_to_edge = "left" } },
  { key = mod .. "+Right",        action = { snap_to_edge = "right" } },
  { key = mod .. "+space",        action = { spawn = { "qs", "ipc", "call", "launcher", "toggle" } } },
  { key = "Alt+Tab",              action = "focus_next" },
  -- escape hatch while a game inhibits shortcuts
  { key = mod .. "+Escape",       action = "toggle_keybinds", allow_when_inhibited = true },
}

for i = 1, 9 do
  table.insert(binds, { key = mod .. "+" .. i,         action = { workspace = i } })
  table.insert(binds, { key = mod .. "+Shift+" .. i,   action = { move_to_workspace = i } })
end

return {
  border = {
    width = 2,
    -- one number for all corners, or per corner
    radius = { top_left = 12, top_right = 12, bottom_right = 4, bottom_left = 4 },
    color = {
      active   = "#89b4fa",
      inactive = "#45475a",
      urgent   = "#f38ba8",
    },
    clip_content = true,             -- round the client's own corners too
    keep_on_maximize = false,
  },

  decorations = "server",            -- "server" (ask clients to drop CSD) | "client"

  outputs = {
    ["DP-1"]   = { mode = "2560x1440@165", position = { 0, 0 }, scale = 1.0,
                   adaptive_sync = "fullscreen" },
    ["eDP-1"]  = { scale = 1.5 },
    ["*"]      = { adaptive_sync = "off" },
  },

  gaming = {
    allow_tearing = "fullscreen",    -- "never" | "fullscreen" | "always" (per client request)
    direct_scanout = true,
    fullscreen_inhibits_idle = true,
  },

  xwayland = {
    enable = true,
    lazy = true,                     -- start on first X11 client
    persistent = false,
    scale = "unscaled",              -- "unscaled" | "native" (HiDPI X11 apps)
  },

  workspaces = { names = { "1", "2", "3", "4", "5", "6", "7", "8", "9" }, initial = 1 },

  focus = {
    follow_mouse = false,
    follow_mouse_requires_movement = true,
    raise_on_focus = true,
    raise_on_focus_delay_ms = 0,
    steal = "smart",                 -- "never" | "smart" (xdg-activation token) | "always"
  },

  placement = { policy = "center", cascade_offset = { 40, 40 } },

  snapping = { range = { inner = 10, outer = 10 }, corner_range = 50, top_maximize = true },
  resistance = { screen_edge = 20, window_edge = 20 },
  gaps = 0,
  margins = { { output = "DP-1", top = 0, bottom = 0, left = 0, right = 0 } },

  regions = {
    { name = "left-third", x = "0%", y = "0%", width = "33%", height = "100%" },
  },

  keyboard = {
    layout = "us,hu", variant = "", options = "grp:alt_shift_toggle",
    repeat_rate = 25, repeat_delay = 600, numlock = true,
    layout_scope = "global",         -- "global" | "window"
    binds = binds,
  },

  mouse = {
    double_click_ms = 500,
    binds = {
      { button = mod .. "+Left",  context = "window", action = "move" },
      { button = mod .. "+Right", context = "window", action = "resize" },
      { button = "Left",          context = "border", action = "resize" },
    },
  },

  libinput = {
    ["*"]          = { accel_profile = "flat" },
    ["touchpad"]   = { tap = true, natural_scroll = true, disable_while_typing = true },
    ["Logitech G502"] = { pointer_speed = -0.3 },
  },

  window_rules = {
    { match = { app_id = "steam_app_*" },    border = { width = 0 }, tearing = true },
    { match = { app_id = "foot" },           border = { radius = 6 } },
    { match = { title = "Picture-in-Picture" }, always_on_top = true, omnipresent = true },
    { match = { class = "gamescope" },       decorations = "none", fullscreen = true },
  },

  environment = {
    XCURSOR_THEME = "Bibata-Modern-Ice",
    XCURSOR_SIZE  = "24",
  },

  autostart = {
    { "qs" },
    { "wl-paste", "--watch", "cliphist", "store" },
  },

  error_command = { "notify-send", "wl-hex", "config error: %s" },
}
```

Same thing in JSON (excerpt):

```json
{
  "border": {
    "width": 2,
    "radius": { "top_left": 12, "top_right": 12, "bottom_right": 4, "bottom_left": 4 },
    "color": { "active": "#89b4fa", "inactive": "#45475a" }
  },
  "keyboard": {
    "binds": [
      { "key": "Super+Return", "action": { "spawn": ["foot"] } },
      { "key": "Super+1", "action": { "workspace": 1 } }
    ]
  }
}
```

## Notes

- `radius` accepts a number (all corners) or a table with the four named
  corners. Missing corners in a table default to 0.
- Colours accept `#rrggbb` and `#rrggbbaa`.
- Keybind flags (from labwc): `layout_dependent`, `on_release`,
  `allow_when_locked`, plus `allow_when_inhibited` for shortcut-inhibit
  escape binds.
- Actions are either a bare string (`"close"`) or a single-key table
  (`{ workspace = 3 }`). The full list lives in `action.rs` and is derived
  from labwc's (see [labwc-survey.md](labwc-survey.md#actions)).
- Window rule matching uses globs on `app_id`, `title`, `class` (X11),
  `instance` (X11), `sandbox_engine`, `tag` (xdg-toplevel-tag).
- Nix: Home Manager `settings` is converted with `lib.generators.toLua` (or
  `builtins.toJSON` if `format = "json"`), so everything above is settable
  from Nix. See [nix.md](nix.md).
