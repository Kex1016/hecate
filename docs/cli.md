# Command line

One binary, `hecate`. Without a subcommand it starts the compositor;
everything else is a subcommand. All `msg` commands talk to the running
compositor over `$HECATE_SOCKET` ([ipc.md](ipc.md)).

Output defaults to readable tables; every query takes `--json` for scripts
and `--format '<template>'` for one-liners (`--format '{id} {app_id}
{title}'`).

## Compositor and config

| command | does |
| ------- | ---- |
| `hecate` | start the compositor (backend picked automatically: DRM on a TTY, winit when nested) |
| `hecate --backend winit\|udev\|headless` | force a backend |
| `hecate --config <path>` | use another config file |
| `hecate validate [path]` | check a config without starting; exit code for CI and the Nix build |
| `hecate schema` | print the config JSON Schema (editor completion) |
| `hecate print-default-config` | full default config as TOML |
| `hecate check` | environment diagnostics: GPU and DRM driver, KMS planes, VRR / async page flip / explicit sync support per connector, libseat/logind session, xwayland-satellite found, portals config, `XDG_*` and `WAYLAND_*` vars, config validity. The first thing to run when something's off on a new machine |
| `hecate completions bash\|zsh\|fish` | shell completions (the Nix package installs them) |
| `hecate --version` | version, git rev, Smithay version, enabled features |

Man pages (`hecate(1)`, `hecate-config(5)`, `hecate-msg(1)`) are generated
from the same clap definitions and schema.

## Identifying windows

| command | does |
| ------- | ---- |
| `hecate msg pick` | crosshair cursor, click a window, prints everything about it: id, app_id, title, pid, X11 or Wayland (and WM_CLASS / instance via satellite), sandbox engine (flatpak), toplevel tag, workspace, output, geometry, stacking layer, states (focused, maximized, fullscreen, always-on-top, omnipresent, minimized), content type, tearing request, idle inhibitor, shortcut inhibitor, border/decoration mode, and **which window rules matched**. `Esc` cancels |
| `hecate msg pick --rule` | same click, prints a ready-to-paste window rule matching that window, in TOML |
| `hecate msg pick --rule --nix` | the same rule as a Nix attrset for `settings.window_rules` |
| `hecate msg pick --layer` | pick a layer-shell surface instead (namespace, layer, anchors, exclusive zone, keyboard mode, matched layer rules) |
| `hecate msg pick --output` | click to get the output under the cursor |
| `hecate msg pick --point` | click to print global and output-local coordinates |
| `hecate msg pick --color` | click to print the pixel colour (`#rrggbb`), optionally `--copy` to the clipboard |
| `hecate msg windows` | all windows, one per line, grouped by output and workspace |
| `hecate msg window <selector>` | full details for one window, like `pick` |
| `hecate msg focused-window` | details for the focused window |
| `hecate msg rules <selector>` | which window rules match, in order, and the final effective properties |
| `hecate msg tree` | the whole stack as a tree: outputs → layers → workspaces → windows → popups, with z-order |
| `hecate msg layers` | layer-shell surfaces with namespaces (useful for writing layer rules) |
| `hecate msg identify` | briefly flashes each window's border and draws its id and app_id on top, so you can match the list to the screen |

### Selectors

Every command that acts on a window takes a selector; without one it uses
the focused window.

| selector | matches |
| -------- | ------- |
| `--id 42` | window id (stable for the window's lifetime) |
| `--app-id 'zen*'` | glob on app_id (X11: WM_CLASS) |
| `--title 'Picture-in-Picture'` | glob on title |
| `--pid 1234` | process id |
| `--workspace 3` / `--output DP-1` | narrow to a workspace or output |
| `--pick` | interactive click |
| `--all` | every match instead of erroring when more than one matches |

## Acting on windows, workspaces and outputs

`hecate msg action <action> [args] [selector]` runs any action from the
action list, same names and arguments as binds. Shortcuts for the common
ones:

| command | does |
| ------- | ---- |
| `hecate msg focus <selector>` | focus (and raise) |
| `hecate msg close <selector>` / `kill <selector>` | close politely / kill the client (`kill --pick` is xkill) |
| `hecate msg move-to-workspace <n> <selector>` | |
| `hecate msg move-to-output <name> <selector>` | |
| `hecate msg set-geometry <selector> --x --y --width --height` | exact placement |
| `hecate msg workspace <n\|next\|prev\|name>` | switch workspace |
| `hecate msg outputs` | outputs with mode, refresh, scale, transform, position, VRR state |
| `hecate msg output <name> mode 2560x1080@200 \| scale 1.5 \| transform 90 \| position 0,0 \| on \| off` | runtime output changes (not persisted, like wlr-randr) |
| `hecate msg workspaces` | workspaces per output with window counts |
| `hecate msg keyboard-layout next\|prev\|<n>` | switch layout |

## Debugging and tuning

| command | does |
| ------- | ---- |
| `hecate msg watch [--filter windows,workspaces,...]` | pretty-printed live event stream |
| `hecate msg event-stream` | raw JSON event stream |
| `hecate msg keys` | while running, prints every key and button press as hecate sees it (keysym, keycode, modifiers, the bind string to write in the config, and which bind fired if any). Like `wev`, but in the compositor's own naming |
| `hecate msg binds [--conflicts]` | resolved binds after base/shell/personal layering; `--conflicts` shows what overrode what |
| `hecate msg gaming` | per output: refresh, VRR on/off, tearing active, direct scanout active (and why not if not: overlapping surface, rounded clip, animation, unsupported format), explicit sync in use; per window: content type, tearing request, pointer lock/confine. The first thing to check when a game feels off |
| `hecate msg frames [--output DP-1]` | frame timing: render time (split into scene, draw and queue), presentation latency, missed frames |
| `hecate msg debug damage\|opaque\|clip\|scanout on\|off` | visual overlays: damage regions, opaque regions, clip shapes, which planes are scanned out |
| `hecate msg log-level debug\|info\|warn` | change log verbosity at runtime |
| `hecate msg set <path> <value>` / `get <path>` / `unset <path>\|--all` | runtime config overrides (Game Mode uses these, see [animations.md](animations.md)) |
| `hecate msg config [--effective]` | the config as loaded, or with overrides applied |
| `hecate msg reload` | reload the config file |
| `hecate msg version` | compositor and IPC version |
| `hecate msg clients` | connected Wayland clients: pid, executable, security context (sandboxed or not), bound globals. Helps when an app misbehaves |
| `hecate msg protocols` | globals hecate advertises, with versions |

## Session

| command | does |
| ------- | ---- |
| `hecate msg exit` | quit the session cleanly (stops `graphical-session.target`, with `hecate-session.target` and the units that are part of it) |
| `hecate msg spawn -- <cmd...>` | run a command with the compositor's environment (`WAYLAND_DISPLAY`, `DISPLAY`, ...) |
| `hecate msg action power_off_outputs` / `power_on_outputs` | DPMS all outputs (idle scripts); any input turns them back on |
