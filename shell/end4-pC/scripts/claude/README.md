# Claude Code hook bridge

Claude Code sessions report to the shell through their hooks, and a permission prompt can be
answered from the shell.

```
Claude Code ── hook input on stdin ──> hook.py ── Unix socket ──> bridge.py ── stdio ──> ClaudeSessions.qml
            <── decision on stdout ───         <── decision ─────           <── allow / deny
                                                                   └── notify-send (category "permission")
```

- `hook.py` is what Claude Code's hook entries run, once per event. It is short-lived and has
  no dependencies beyond Python's standard library.
- `bridge.py` is a child of the shell (`services/claude/ClaudeSessions.qml` starts it). It owns
  the socket, keeps the session list, holds the hook of a permission request open until
  someone answers, and sends the request as a desktop notification.
- `ClaudeSessions.qml` (`import qs.services.claude`) is what panels, toasts and widgets use.

Everything is local: a Unix socket, pipes and the session D-Bus. Nothing uses the network.

## The safety rule

Nothing is ever allowed automatically. `hook.py` is the only thing that speaks to Claude Code,
and it prints a decision in exactly one case: the hook event is `PermissionRequest`, and the
bridge sent a well-formed `decision` for that request on the same connection.

In every other case it prints nothing and exits 0, which Claude Code reads as "no decision":
its own permission prompt in the terminal stands. That covers the shell or the bridge not
running, no socket, a socket or directory that is not the user's, a peer that is not the user,
a refused or broken connection, no `pending` in time, no answer in time, a closed connection,
a line that is not JSON, an unknown message type, a wrong request id, an unknown `behavior`,
and any exception. `hook.py` never exits 2 and never prints `allow` on its own.

"Always" only ever applies a rule Claude Code itself suggested for that request
(`permission_suggestions` in the hook's own input). The bridge picks one by index; it cannot
send a rule.

Answers reach the bridge from the shell process only: its stdin, and the action of the
notification it sent. A program that connects to the socket can report events but cannot
answer a request.

## Which hook events

`PermissionRequest` is the event that fires when Claude Code is about to ask for permission,
and its `decision` output is how a hook answers on the user's behalf. `PreToolUse` fires
before every tool call, allowed or not, so it is used for the activity line only and never
returns a decision.

| Event | Used for | Answered? |
| --- | --- | --- |
| `PermissionRequest` | a pending request; the session is `waiting` | yes: allow, deny or nothing |
| `SessionStart`, `SessionEnd` | the session list | no |
| `UserPromptSubmit` | the session is `working` | no |
| `PreToolUse`, `PostToolUse`, `PostToolUseFailure` | `activity`; a request answered in the terminal leaves the list | no |
| `Notification` | `waiting` for input (`permission_prompt`, `elicitation_dialog`, ...), `message` | no |
| `Stop`, `StopFailure` | the session is `done`, with `summary` or `error` | no |
| `SubagentStart`, `SubagentStop` | `subagents` | no |

Events that are not answered should be registered with `"async": true`, so they never hold a
tool call up. `hooks.example.json` is a complete `hooks` block.

## Installing the hooks

The `hooks` block goes in a Claude Code settings file (`~/.claude/settings.json`, or a managed
one). `hook.py` needs only a Python 3 interpreter, which may not be on the terminal's `PATH`,
so name both by absolute path. From Nix, with `shell` being the `hecate-shell` package:

```nix
command = "${shell.pythonEnv}/bin/python3";
args = [ "${shell}/${shell.config}/scripts/claude/hook.py" ];
```

Every event runs the same command; the script reads the event's name from its input.
`PermissionRequest` needs a long `timeout` (the hook waits for the answer; Claude Code's
default is 600 seconds). `Stop`, `StopFailure` and `SessionEnd` are not async because an async
hook still running when a session exits is killed.

## The socket

`$XDG_RUNTIME_DIR/hecate-claude/bridge.sock`, a Unix stream socket. The directory is created
with mode 0700 and the socket with 0600, both owned by the user. `hook.py` refuses a directory
or socket that is not owned by its own user or that others can write to, and both ends check
the peer's uid (`SO_PEERCRED`). `HECATE_CLAUDE_SOCKET` overrides the path (tests).

Messages are UTF-8 JSON objects, one per line, at most 1 MiB each. Every message has
`"v": 1` and a `"type"`.

### hook.py to bridge.py

One message per connection, sent first:

```json
{
  "v": 1,
  "type": "hook",
  "id": "5f0c1d3e9a7b4c2d8e6f0a1b2c3d4e5f",
  "reply": true,
  "input": { "session_id": "abc123", "hook_event_name": "PermissionRequest", "cwd": "/home/cakes/src/cakeos",
             "permission_mode": "default", "tool_name": "Bash",
             "tool_input": { "command": "nix build .#nixosConfigurations.cakeos.config.system.build.toplevel" },
             "permission_suggestions": [ { "type": "addRules", "rules": [ { "toolName": "Bash", "ruleContent": "nix build:*" } ],
                                           "behavior": "allow", "destination": "localSettings" } ] },
  "client": { "pid": 5120, "claude_pid": 5100, "tty": "/dev/pts/3",
              "terminal": { "name": "kitty", "pid": 4242, "window_id": "7" } }
}
```

- `id`: 32 hex characters, random, made by the hook. It names the request.
- `reply`: `true` only for `PermissionRequest`; the hook then waits. For every other event it
  is `false` and the hook closes the connection right after the line.
- `input`: Claude Code's hook input, as it arrived on stdin.
- `client`: who is asking. `claude_pid` is the Claude Code process, `terminal` the terminal
  emulator found among its ancestors (for "go to terminal"); fields are 0 or empty when unknown.

### bridge.py to hook.py

Only when `reply` was `true`:

```json
{ "v": 1, "type": "pending", "id": "5f0c…" }
```

The bridge has the request and is showing it. It must arrive within 2 seconds, or the hook
gives up and prints nothing.

```json
{ "v": 1, "type": "decision", "id": "5f0c…", "behavior": "allow" }
{ "v": 1, "type": "decision", "id": "5f0c…", "behavior": "allow", "suggestion": 0 }
{ "v": 1, "type": "decision", "id": "5f0c…", "behavior": "deny", "message": "Denied from the desktop." }
```

- `behavior`: `"allow"` or `"deny"`, nothing else.
- `suggestion` (allow only): index into the input's `permission_suggestions`; the hook echoes
  that entry as `updatedPermissions` if it is an `addRules` entry with `behavior: "allow"`,
  and otherwise allows this one call only.
- `message` (deny only): shown to Claude.

```json
{ "v": 1, "type": "pass", "id": "5f0c…", "reason": "handed back" }
```

No decision: the hook prints nothing and the prompt in the terminal stands. Closing the
connection means the same.

When the hook goes away first (Claude Code cancels it once the prompt is answered in the
terminal, or its timeout runs out), the bridge sees the connection close and drops the request.

### What hook.py prints

For an allow:

```json
{"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}}
```

with `"updatedPermissions": [ <the suggested entry> ]` inside `decision` for "always". For a deny:

```json
{"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "Denied from the desktop."}}}
```

Otherwise nothing. The exit code is always 0.

## bridge.py and the shell

`ClaudeSessions.qml` runs `python3 bridge.py` and talks to it over its stdin and stdout, one
JSON object per line. When its stdin closes (the shell went away) the bridge passes every
pending request back to its terminal, removes the socket and exits.

bridge.py to the shell:

```json
{ "type": "hello", "v": 1, "socket": "/run/user/1000/hecate-claude/bridge.sock" }
{ "type": "state", "sessions": [ … ], "requests": [ … ] }
{ "type": "event", "name": "permissionRequested", "request": { … } }
{ "type": "event", "name": "permissionResolved", "id": "5f0c…", "outcome": "allow" }
{ "type": "event", "name": "sessionStarted", "session": { … } }
{ "type": "event", "name": "sessionDone", "session": { … } }
{ "type": "event", "name": "sessionEnded", "id": "abc123" }
{ "type": "event", "name": "notified", "session": { … }, "notificationType": "idle_prompt", "message": "…" }
{ "type": "event", "name": "look", "session": "abc123" }
```

`state` is the whole list, sent after every change. The session and request objects are
described field by field in `ClaudeSessions.qml`.

The shell to bridge.py:

```json
{ "type": "decide", "id": "5f0c…", "behavior": "allow", "scope": "once" }
{ "type": "decide", "id": "5f0c…", "behavior": "allow", "scope": "always" }
{ "type": "decide", "id": "5f0c…", "behavior": "deny", "message": "" }
{ "type": "pass", "id": "5f0c…" }
{ "type": "dismiss", "session": "abc123" }
{ "type": "config", "notifications": true }
```

## The desktop notification

Each permission request is also sent with `notify-send`: app name `claude`, category
`permission`, urgency critical, summary `~/src/cakeos wants to run nix build`, the command
or path as the body, and the actions `allow`, `deny` and `look`. The shell's toast shows it in
the gold frame and it lands in the notification history. `allow` and `deny` answer the
request, `look` focuses the session's terminal, and closing the toast leaves the request for
the panel or the terminal. The notification is withdrawn when the request is answered
somewhere else.

## Tests

```
python3 -m unittest discover -s shell/end4-pC/scripts/claude/tests
```
