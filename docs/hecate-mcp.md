# hecate-mcp

`hecate-mcp` is a Model Context Protocol server for hecate. It lets an
MCP client, such as Claude Code, look at the desktop: which windows are
open, where they are, what the outputs and workspaces are, what the shell's
notifications said, and a screenshot of any of them. With permission it can
also act: focus a window, move it, tint its border, send a notification.

It is a separate binary from `hecate` (`packages.hecate-mcp` in the flake,
also `hecate-mcp` in the Nix package's `bin`). It speaks newline-delimited
JSON-RPC 2.0 over stdio, tools only, and understands protocol versions
2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05.

## Connecting

It finds the compositor's socket from `$HECATE_SOCKET`, or else the live
`hecate.*.sock` in `$XDG_RUNTIME_DIR`, so it works from a terminal that was
started outside the session's environment.

```sh
claude mcp add hecate -- hecate-mcp --allow tint
```

`claude mcp list` should then show `hecate` as connected. Tools appear as
`mcp__hecate__<name>`.

## Tools

Read-only tools are on by default.

| tool | what it returns |
| ---- | --------------- |
| `list_windows` | windows: id, app id, title, workspace, output, geometry, state, `border_tint`, `focus_serial`. Filters for app id, title, workspace and output |
| `get_window` | details for one window, by id, app id glob, title glob or pid (the focused window when none is given) |
| `list_workspaces` | workspaces |
| `list_outputs` | outputs: mode, logical position and size, scale, VRR |
| `list_notifications` | the shell's notification history, newest first, with limit, app and text filters |
| `screenshot` | grim capture of every output, one output, or one window's area. Scaled to a 1568 px longest edge by default, JPEG or PNG on request. A minimized window, or one on a workspace that isn't shown, is refused |

Tools that change something are hidden from `tools/list` and refused unless
their permission is allowed:

| tool | permission |
| ---- | ---------- |
| `focus_window`, `move_window_to_workspace`, `switch_workspace` | `windows` |
| `set_border_tint` | `tint` |
| `send_notification` (through `notify-send`) | `notify` |

Calling a tool with an argument it doesn't know is an error, not silently
ignored. Every result carries `structuredContent` and the same JSON as text.

## Permissions

Pass them on the command line, or in the config file:

- `--allow windows,tint,notify`, or `--allow all`
- `--read-only`: leaves every changing tool out, whatever else is allowed
- `--no-screenshots`: turns `screenshot` off

`~/.config/hecate/mcp.toml` takes the same settings:

```toml
allow = ["tint"]        # windows, tint, notify
screenshots = true
notifications_file = "~/.cache/quickshell/notifications/notifications.json"
grim = "/run/current-system/sw/bin/grim"
notify_send = "notify-send"
```

Other flags: `--socket <path>` to pick the compositor socket, `--config
<path>` for another config file, `--list-tools` to print what would be
offered and exit.

Keep it read-only unless you want Claude to move windows or change a
border. `tint` is the smallest permission that changes anything, and the
tint names match the ones the config gives (see
[config.md](config.md#haiiro-look)).

## Where it reads from

- Windows, workspaces, outputs and tints come from the compositor's IPC
  socket, the same requests `hecate msg` sends (see [ipc.md](ipc.md)).
- Notifications come from the shell's history file
  (`~/.cache/quickshell/notifications/notifications.json` by default), not
  from the compositor.
- Screenshots come from `grim` on `PATH`, or the path in `mcp.toml`.

## Not verified yet

Sending notifications through a real daemon, the workspace tools against a
live compositor, and screenshots with a real grim were only checked by
their request shapes and by fake grim and notify-send scripts in the test
suite. Run `hecate-mcp --list-tools` and a `screenshot` in a real session
before relying on them.
