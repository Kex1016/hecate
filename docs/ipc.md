# IPC

Wayland protocols stay the primary interface; anything a protocol covers
(toplevel list, workspaces, outputs config, idle, lock) is done through
the protocol. The socket covers the rest: scripting, shell integration
(the bundled end4-pC shell's `HecateBackend`, see [shell.md](shell.md)) and
debugging.

## Transport

- Unix stream socket at `$XDG_RUNTIME_DIR/hecate.$WAYLAND_DISPLAY.$PID.sock`,
  exported as `$HECATE_SOCKET` to children and the systemd/dbus activation
  environment.
- Newline-delimited JSON. One request per line, one reply per line.
  Same model as niri's IPC, which shells already know how to talk to.
- **niri-shaped payloads** for `windows`, `workspaces`, `outputs` and the
  event stream: same field names (`id`, `app_id`, `title`, `workspace_id`,
  `is_focused`, `is_active`, `output`, `idx`, `name`, `logical {x, y,
  width, height, scale, transform}`, `current_mode`, `modes`,
  `layout.window_size`) wherever the concept exists in hecate. end4-pC's
  `NiriBackend.qml` parses exactly these, so its hecate backend is a near
  copy. hecate-only fields are added alongside (`geometry`, `stacking`,
  `always_on_top`, `omnipresent`, `is_x11`). This is shape compatibility,
  not niri IPC emulation: different socket, different binary, no niri
  actions.
- Only the session user can connect (socket mode `0600`, peer cred check).
- Versioned: `{"version": 1}` in the handshake reply; additive changes only
  within a version.

## Requests

| request                   | reply                                        |
| ------------------------- | -------------------------------------------- |
| `"version"`               | compositor version, IPC version              |
| `{"action": <Action>}`    | ok / error. Same `Action` enum and syntax as binds |
| `"outputs"`               | name, make/model, mode, scale, transform, position, VRR, focused |
| `"focused_output"`        | output or null                               |
| `"workspaces"`            | id, name, output (per_output mode), active, urgent, window ids |
| `"windows"`               | id, app_id, title, workspace id, output, geometry, state flags, is_x11, pid |
| `"focused_window"`        | window or null                               |
| `"keyboard_layouts"`      | names, current index                         |
| `"layers"`                | layer-shell surfaces: namespace, layer, output, exclusive zone, keyboard mode |
| `"cursor_position"`       | x, y in global logical coords, output under the cursor |
| `"binds"`                 | the resolved bind list (for hotkey cheat sheets) |
| `"config"`                | effective config as JSON                     |
| `{"reload": {}}`          | ok / validation errors                       |
| `{"validate": {"path": ...}}` | validation errors without applying       |
| `"event_stream"`          | switches the connection to an event stream (below) |

Window and workspace ids match the ids used in ext-workspace / foreign
toplevel where possible, so a shell can correlate protocol objects with
IPC data (Noctalia's niri backend does the same with niri ids).

Actions the shell needs that aren't window management (all go through
`{"action": ...}` like everything else): `switch_keyboard_layout
next|prev|<index>`, `set_cursor_theme <name> <size>` (re-exports
`XCURSOR_*` and updates the default cursor), `power_off_outputs` /
`power_on_outputs` (same effect as wlr-output-power), `focus_output
<name>`, `move_cursor <x> <y>`.

## Events

After `"event_stream"`, the server first sends full state
(`workspaces_changed`, `windows_changed`, `outputs_changed`,
`keyboard_layouts_changed`), then incremental events:
`window_opened_or_changed`, `window_closed`, `window_focus_changed`,
`workspace_activated`, `workspace_urgency_changed`, `output_focus_changed`,
`keyboard_layout_switched`, `config_reloaded`.

## CLI

```
hecate msg version
hecate msg action close
hecate msg action workspace 3
hecate msg windows            # human table
hecate msg --json windows     # raw JSON
hecate msg event-stream
hecate msg reload
```

## Not in scope

- Running arbitrary code in the compositor, plugins, or a scripting API.
- Emulating Hyprland, Sway/i3 or niri IPC. Shells get a native hecate
  backend instead (bundled end4-pC first, Noctalia maybe later).
