# IPC

Wayland protocols stay the primary interface; anything a protocol covers
(toplevel list, workspaces, outputs config, idle, lock) is done through
the protocol. The socket covers the rest: scripting, shell integration
(Noctalia's `Unknown` gaps, see PLAN.md section 6) and debugging.

## Transport

- Unix stream socket at `$XDG_RUNTIME_DIR/wl-hex.$WAYLAND_DISPLAY.$PID.sock`,
  exported as `$WL_HEX_SOCKET` to children and the systemd/dbus activation
  environment.
- Newline-delimited JSON. One request per line, one reply per line.
  Same model as niri's IPC, which shells already know how to talk to.
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
| `"binds"`                 | the resolved bind list (for hotkey cheat sheets) |
| `"config"`                | effective config as JSON                     |
| `{"reload": {}}`          | ok / validation errors                       |
| `"event_stream"`          | switches the connection to an event stream (below) |

Window and workspace ids match the ids used in ext-workspace / foreign
toplevel where possible, so a shell can correlate protocol objects with
IPC data (Noctalia's niri backend does the same with niri ids).

## Events

After `"event_stream"`, the server first sends full state
(`workspaces_changed`, `windows_changed`, `outputs_changed`,
`keyboard_layouts_changed`), then incremental events:
`window_opened_or_changed`, `window_closed`, `window_focus_changed`,
`workspace_activated`, `workspace_urgency_changed`, `output_focus_changed`,
`keyboard_layout_switched`, `config_reloaded`.

## CLI

```
wl-hex msg version
wl-hex msg action close
wl-hex msg action workspace 3
wl-hex msg windows            # human table
wl-hex msg --json windows     # raw JSON
wl-hex msg event-stream
wl-hex msg reload
```

## Not in scope

- Running arbitrary code in the compositor, plugins, or a scripting API.
- Emulating Hyprland, Sway/i3 or niri IPC. Shells get a native wl-hex
  backend instead (see the Noctalia plan).
