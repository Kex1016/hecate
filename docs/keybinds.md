# Default keybinds

Built from labwc's defaults (`README.md` usage table, `docs/rc.xml.all`)
and your niri binds (`universe/modules/wms/niri/home/niri.nix`). niri is a
scrolling tiler, so column actions are translated to their nearest stacking
equivalent or dropped.

`Mod` = `Super` on a real session, `Alt` when running nested (like niri).
`J K L M` mirror your niri layout (left, down, up, right).

## Defaults (ship with the compositor)

### Windows

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+Q`                      | `close`                        | niri |
| `Alt+F4`                     | `close`                        | labwc |
| `Mod+Left/Down/Up/Right`     | `focus_direction <dir>`        | niri `focus-column-left` / `focus-window-down` ... |
| `Mod+J/K/L/M`                | `focus_direction <dir>`        | niri |
| `Mod+Ctrl+Left` / `J`        | `snap_to_edge left`            | niri `move-column-left` → stacking: snap left half (labwc `Super+Left`) |
| `Mod+Ctrl+Right` / `M`       | `snap_to_edge right`           | as above |
| `Mod+Ctrl+Up` / `L`          | `toggle_maximize`              | labwc `Super+Up` snaps to top = maximize |
| `Mod+Ctrl+Down` / `K`        | `unsnap` (restore size)        | labwc `Super+Down` |
| `Mod+Shift+Left/Down/Up/Right` (+ `JKLM`) | `focus_output <dir>` | niri `focus-monitor-*` |
| `Mod+Shift+Ctrl+Left/Down/Up/Right` (+ `JKLM`) | `move_to_output <dir>` | niri `move-window-to-monitor-*` |
| `Mod+F`                      | `toggle_maximize`              | niri `maximize-column` |
| `Mod+Shift+F`                | `toggle_fullscreen`            | niri |
| `Mod+Ctrl+F`                 | `toggle_maximize horizontal`   | niri `expand-column-to-available-width` |
| `Mod+C`                      | `center`                       | niri `center-column` |
| `Mod+R`                      | `cycle_width_preset`           | niri `switch-preset-column-width` |
| `Mod+Shift+R`                | `cycle_height_preset`          | niri `switch-preset-window-height` |
| `Mod+Ctrl+R`                 | `reset_size`                   | niri `reset-window-height` |
| `Mod+KP_5` / `Mod+KP_6`      | `resize_relative width -10%/+10%` | niri `set-column-width` |
| `Mod+Shift+KP_5` / `KP_6`    | `resize_relative height -10%/+10%` | niri `set-window-height` |
| `Mod+G`                      | `toggle_always_on_top`         | niri `switch-focus-between-floating-and-tiling` (repurposed, no tiling here) |
| `Mod+Shift+G`                | `toggle_omnipresent`           | niri `toggle-window-floating` (repurposed) |
| `Alt+Tab` / `Alt+Shift+Tab`  | `focus_next` / `focus_prev` (no OSD) | labwc |
| `Mod+Shift+Escape`           | `toggle_keybinds` (works while a game inhibits shortcuts) | labwc `ToggleKeybinds` |

### Workspaces

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+1..9`                   | `workspace <n>`                | niri |
| `Mod+Shift+1..9`             | `move_to_workspace <n>`        | niri `move-window-to-workspace` |
| `Mod+U` / `Mod+Page_Down` / `Mod+WheelScrollDown` | `workspace next` | niri |
| `Mod+I` / `Mod+Page_Up` / `Mod+WheelScrollUp`     | `workspace prev` | niri |
| `Mod+Ctrl+U` / `Page_Down`   | `move_to_workspace next`       | niri `move-window-to-workspace-down` |
| `Mod+Ctrl+I` / `Page_Up`     | `move_to_workspace prev`       | niri |
| `Mod+Shift+U` / `Mod+Shift+I` | `move_workspace_to_output next/prev` (only in `per_output` mode) | niri `move-workspace-down/up` reorders; fixed workspaces can't be reordered, so this moves the workspace across outputs instead |
| `Mod+D` *(labwc)*            | not bound by default           | labwc's `Super+D` show-desktop clashes with your launcher; `toggle_show_desktop` stays available |

### Session and system

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+Return`                 | `spawn $TERMINAL` (fallback `foot`, `kitty`, `alacritty`) | both |
| `Mod+Shift+E`                | `exit`                         | niri default |
| `Mod+Shift+C`                | `reload`                       | new |
| `XF86AudioRaiseVolume` / `LowerVolume` / `Mute` / `MicMute` | `wpctl` | labwc (it uses `pactl`; `wpctl` fits PipeWire) |
| `XF86MonBrightnessUp/Down`   | `brightnessctl set +10%/10%-`  | labwc |
| `XF86AudioPlay/Pause/Next/Prev` | `playerctl`                 | new |

### Mouse

| bind                         | context  | action      | origin |
| ---------------------------- | -------- | ----------- | ------ |
| `Mod+Left` drag              | window   | `move`      | labwc |
| `Mod+Right` drag             | window   | `resize`    | labwc |
| `Left` drag                  | border   | `resize`    | labwc |
| `Left` click                 | window   | focus + raise | labwc |

## Dropped from your niri binds

| niri bind | why |
| --------- | --- |
| `Mod+Home/End` (focus first/last column), `Mod+Ctrl+Home/End` | no columns |
| `Mod+Comma` / `Mod+semicolon` (consume/expel) | no columns |
| `Mod+Space` (tabbed column) | no columns. Free for the shell |
| `Mod+A` (overview) | no built-in overview. The shell can build one from screencopy; bind it to `qs ipc call ...` |
| `Mod+colon` (hotkey overlay) | no built-in overlay. `wl-hex msg binds` returns the bind list as JSON so the shell can draw a cheat sheet |

## Your personal binds (example config, not defaults)

These go in your Home Manager `settings`, as in [config.md](config.md):

| bind | action |
| ---- | ------ |
| `Mod+Return` | `spawn kitty` |
| `Mod+D` | `noctalia-shell ipc call launcher toggle` |
| `Mod+T` / `Mod+B` / `Mod+E` | `nautilus` / `zen-beta` / `codium` |
| `Mod+Backspace` | `noctalia-shell ipc call lockScreen lock` |
| `Mod+X` | `noctalia-shell ipc call sessionMenu toggle` |
| `Mod+V` | `noctalia-shell ipc call launcher clipboard` |
| `Mod+N` | `noctalia-shell ipc call notifications toggleHistory` |
| `Mod+S` | `~/.local/bin/hyprcap shot region -zcw` |
| `Mod+A` | your shell's overview, if it has one |
| `XF86Audio*`, `XF86MonBrightness*` | `noctalia-shell ipc call volume ...` / `brightness ...` / `media ...` (overrides the defaults) |
