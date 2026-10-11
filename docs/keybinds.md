# Keybinds

Three layers, later wins:

1. **base**: compositor defaults, built from labwc's defaults and your niri
   binds. Shipped in the binary.
2. **shell**: end-4's illogical-impulse binds, translated from
   `dots-hyprland/dots/.config/hypr/hyprland/keybinds.lua`. Added by the
   Home Manager module when `shell.enable = true`.
3. **personal**: your own `settings.binds`.

`Mod` = `Super` on a real session, `Alt` when running nested (like niri).

## base

niri is a scrolling tiler, so column actions are translated to their
nearest stacking equivalent or dropped.

### Windows

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+Q`                      | `close`                        | niri, end-4 |
| `Alt+F4`                     | `close`                        | labwc |
| `Mod+Left/Down/Up/Right`     | `focus_direction <dir>` (stops at the output edge) | niri `focus-column-left` ... |
| `Mod+J/K/L/M`                | `focus_direction <dir>`        | niri. **Dropped in the shell profile**, see conflicts |
| `Mod+Ctrl+Left` / `Right`    | `snap_to_edge left/right`      | niri `move-column-*` → snap half (labwc `Super+Left`) |
| `Mod+Ctrl+Up`                | `toggle_maximize`              | labwc `Super+Up` |
| `Mod+Ctrl+Down`              | `unsnap` (restore size)        | labwc `Super+Down` |
| `Mod+Shift+Left/Down/Up/Right` | `focus_output <dir>`         | niri `focus-monitor-*` |
| `Mod+Shift+Ctrl+Left/Down/Up/Right` | `move_to_output <dir>`  | niri `move-window-to-monitor-*` |
| `Mod+F`                      | `toggle_maximize`              | niri `maximize-column`. **Shell profile changes it**, see conflicts |
| `Mod+Shift+F`                | `toggle_fullscreen`            | niri |
| `Mod+Ctrl+F`                 | `toggle_maximize horizontal`   | niri `expand-column-to-available-width` |
| `Mod+C`                      | `center`                       | niri `center-column` |
| `Mod+R`                      | `cycle_width_preset`           | niri `switch-preset-column-width` |
| `Mod+Semicolon`              | `cycle_height_preset`          | niri `switch-preset-window-height` (moved off `Mod+Shift+R`, see conflicts) |
| `Mod+Apostrophe`             | `reset_size`                   | niri `reset-window-height` (moved off `Mod+Ctrl+R`) |
| `Mod+KP_5` / `Mod+KP_6`      | `resize_relative width -10%/+10%` | niri `set-column-width` |
| `Mod+Shift+KP_5` / `KP_6`    | `resize_relative height -10%/+10%` | niri `set-window-height` |
| `Mod+P`                      | `toggle_omnipresent`           | end-4 "pin" (Hyprland pin = visible on all workspaces) |
| `Mod+Alt+Space`              | `toggle_always_on_top`         | end-4's float toggle slot; no floating/tiling split here |
| `Mod+Shift+Alt+Q`            | `kill`                         | end-4 |
| `Alt+Tab` / `Alt+Shift+Tab`  | `focus_next` / `focus_prev` (no OSD) | labwc |
| `Mod+Shift+Escape`           | `toggle_keybinds` (works while a game inhibits shortcuts) | labwc `ToggleKeybinds` |

### Workspaces

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+1..9`, `Mod+0` (=10)    | `workspace <n>`                | niri, end-4 |
| `Mod+Shift+1..9`, `Mod+Shift+0` | `move_to_workspace <n>`     | niri `move-window-to-workspace` |
| `Mod+U` / `Mod+Page_Down` / `Mod+WheelScrollDown` | `workspace next` | niri |
| `Mod+I` / `Mod+Page_Up` / `Mod+WheelScrollUp`     | `workspace prev` | niri. `Mod+I` clashes with end-4, see conflicts |
| `Mod+Ctrl+U` / `Mod+Ctrl+Page_Down` | `move_to_workspace next` | niri |
| `Mod+Ctrl+I` / `Mod+Ctrl+Page_Up`   | `move_to_workspace prev` | niri |
| `Mod+Shift+U` / `Mod+Shift+I` | `move_workspace_to_output next/prev` (`per_output` scope only) | niri `move-workspace-down/up` |

### Session and system

| bind                         | action                         | origin |
| ---------------------------- | ------------------------------ | ------ |
| `Mod+Return`                 | `spawn $TERMINAL` (fallback `foot`, `kitty`, `alacritty`) | all three |
| `Mod+Shift+E`                | `exit`                         | niri default |
| `Mod+Ctrl+Alt+R`             | `reload`                       | new; `Mod+Shift+C` is end-4's colour picker |
| `Mod+Shift+L`                | `systemctl suspend`            | end-4 |
| `XF86Audio*`                 | `wpctl` / `playerctl`          | labwc, end-4 |
| `XF86MonBrightness*`         | `brightnessctl`                | labwc, end-4 |

### Mouse

| bind                         | context  | action      | origin |
| ---------------------------- | -------- | ----------- | ------ |
| `Mod+BTN_LEFT` drag          | window   | `move`      | labwc, end-4 |
| `Mod+BTN_MIDDLE` drag        | window   | `move`      | end-4 |
| `Mod+BTN_RIGHT` drag         | window   | `resize`    | labwc, end-4 |
| `BTN_LEFT` drag              | border   | `resize`    | labwc |
| `BTN_LEFT` click             | window   | focus + raise | labwc |

## shell (end-4 illogical-impulse)

end-4 uses Hyprland `global` shortcuts (`quickshell:<name>`). On hecate
each becomes a spawn of `hecate-shell-ipc shortcut trigger|press|release <name>`
through the generic `shortcut` IPC target we add to the shell
([shell.md](shell.md)); `hecate-shell-ipc shortcut list` prints every name.
Written below as `shell:<name>`. The profile lives in `nix/shell-binds.nix`
and lands in the config as `shell_binds` (between the base binds and yours),
so `hecate msg binds` shows each bind's layer.

| bind                         | action                                  |
| ---------------------------- | --------------------------------------- |
| `Super` (tap)                | `shell:searchToggleRelease` (launcher / search) |
| `Super_L`, `Super_R` (hold)  | press/release `shell:workspaceNumber` (shows workspace numbers) |
| `Mod+V`                      | `shell:overviewClipboardToggle`         |
| `Mod+Period`                 | `shell:overviewEmojiToggle`             |
| `Mod+A`, `Mod+O`             | `shell:sidebarLeftToggle`               |
| `Mod+Alt+A`                  | `shell:sidebarLeftToggleDetach`         |
| `Mod+N`                      | `shell:sidebarRightToggle` (notifications, quick settings) |
| `Mod+K`                      | `shell:oskToggle`                       |
| `Mod+M`                      | `shell:mediaControlsToggle`             |
| `Mod+G`                      | `shell:overlayToggle`                   |
| `Mod+J`                      | `shell:barToggle`                       |
| `Ctrl+Alt+Delete`            | `shell:sessionToggle`                   |
| `Mod+Ctrl+T` / `Mod+Ctrl+Alt+T` | `shell:wallpaperSelectorToggle` / `wallpaperSelectorRandom` |
| `Mod+Escape`                 | `shell:settingsToggle` (end4-pC README) |
| `Mod+L`                      | lock (`hecate-shell-ipc lock activate`; works while locked) |
| `Mod+F` / `Mod+D`            | `toggle_fullscreen` / `toggle_maximize` (see conflicts) |
| `Mod+Shift+S`, `Print`       | `shell:regionScreenshot`                |
| `Mod+Shift+A`                | `shell:regionSearch` (Google Lens)      |
| `Mod+Shift+X`                | `shell:regionOcr`                       |
| `Mod+Shift+T`                | `shell:screenTranslate`                 |
| `Mod+Shift+R`, `Mod+Alt+R`   | `shell:regionRecord`                    |
| `Mod+Shift+C`                | colour picker (`hyprpicker` works on any wlr-screencopy compositor) |
| `Mod+Ctrl+R`                 | restart the shell (`systemctl --user restart hecate-shell`) |
| `Mod+Shift+N` / `B` / `P` / `M` | media next / previous / play-pause / mute |
| `Mod+Alt+M`                  | mic mute                                |

end-4's app launchers (`Mod+T`, `Mod+E`, `Mod+W`, `Mod+X`, `Mod+I`, ...)
are left out of the shell profile; apps are personal.

Not bound because end4-pC has no such shortcut: `Mod+Tab` (the workspace
overview is off on hecate for now), `Mod+Slash` (end4-pC dropped ii's cheat
sheet; the keybind list in its settings reads `hecate msg -j binds`),
`Mod+Ctrl+Shift+D` and `Mod+Ctrl+P`.

## Conflicts

Between base (labwc + niri), the shell profile and your niri personal binds.
Resolution in the last column, **agreed in round 3**.

| bind | niri / base | end-4 shell | your niri personal | decision |
| ---- | ----------- | ----------- | ------------------ | -------- |
| `Mod+J/K/L/M` | focus left/down/up/right | bar / OSK / lock / media | | **shell**; directional focus stays on the arrow keys |
| `Mod+Shift+J/K/L/M` | focus output | `Shift+L` suspend, `Shift+M` mute | | **shell**; arrows only for output focus |
| `Mod+F` | maximize | fullscreen (`Mod+D` = maximize) | | **shell**: `F` fullscreen, `D` maximize. `Mod+Shift+F` stays fullscreen too |
| `Mod+D` | | maximize | launcher | **shell** (launcher is `Super` tap now) |
| `Mod+G` / `Mod+Shift+G` | always-on-top / omnipresent | widget overlay | | **shell**; those moved to `Mod+Alt+Space` / `Mod+P` |
| `Mod+Shift+R` | height preset | region record | | **shell**; height preset moved to `Mod+Semicolon` |
| `Mod+Ctrl+R` | reset size | restart shell | | **shell**; reset moved to `Mod+Apostrophe` |
| `Mod+Shift+C` | (was reload) | colour picker | | **shell**; reload moved to `Mod+Ctrl+Alt+R` |
| `Mod+A` | (niri overview, dropped) | left sidebar | | **shell** |
| `Mod+N` | | right sidebar | notification history | **shell** (same intent) |
| `Mod+V` | | clipboard | clipboard | **shell** (same intent) |
| `Mod+S` | | Hyprland scratchpad | `hyprcap` screenshot | **personal**: map to `shell:regionScreenshot` (no scratchpad in hecate) |
| `Mod+B` | | left sidebar (alias) | browser | **personal** (drop the alias, `Mod+A`/`Mod+O` remain) |
| `Mod+X` | | text editor | session menu | **personal**: `shell:sessionToggle` |
| `Mod+T` | | terminal | file manager | **personal** |
| `Mod+E` | | file manager | editor (codium) | **personal** |
| `Mod+I` | workspace prev | settings app | | **base** (`Mod+I` workspace prev); settings via `Mod+Escape` |
| `Mod+C` | center window | VS Code | | **base** (your editor is on `Mod+E`) |
| `Mod+Backspace` | | | lock | **personal**, alongside `Mod+L` |

## Your personal binds (example, not defaults)

| bind | action |
| ---- | ------ |
| `Mod+Return` | `spawn kitty` |
| `Mod+T` / `Mod+B` / `Mod+E` | `nautilus` / `zen-beta` / `codium` |
| `Mod+S` | `shell:regionScreenshot` |
| `Mod+X` | `shell:sessionToggle` |
| `Mod+Backspace` | `loginctl lock-session` |

The Noctalia binds from your niri config (`noctalia-shell ipc call ...`)
are replaced by their end-4 equivalents above.
