pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common

/**
 * Every Claude Code session on the machine, as its hooks report it, and the permission
 * requests waiting for an answer.
 *
 *   import qs.services.claude
 *   ClaudeSessions.sessions / .requests / .allow(id) / .deny(id)
 *
 * How it gets here: Claude Code runs scripts/claude/hook.py for its hook events. The hook
 * sends each event over a user-only Unix socket to scripts/claude/bridge.py, a child of this
 * service, which keeps the session list and holds the hook of a permission request open until
 * someone answers. The message formats are in scripts/claude/README.md.
 *
 * Safety: nothing here allows anything by itself. A request is allowed only by allow() or
 * allowAlways(), or by the "allow" action of its desktop notification. When the shell or the
 * bridge is not running, the socket fails, or no answer comes, the hook prints no decision
 * and Claude Code asks in its terminal as it always does. Do not expose allow() to IPC, a
 * global shortcut or anything else a program can call: a session could then approve itself.
 */
Singleton {
    id: root

    // ---- status ----------------------------------------------------------------------------

    // The bridge is running and its socket is listening.
    property bool available: false
    // Where the hook finds the bridge: $XDG_RUNTIME_DIR/hecate-claude/bridge.sock
    property string socketPath: ""
    // What Claude Code's hook entries run: `python3 <hookScript>` (the cakeos option writes them).
    readonly property string hookScript: Quickshell.shellPath("scripts/claude/hook.py")
    readonly property string bridgeScript: Quickshell.shellPath("scripts/claude/bridge.py")

    // Off switches (both default on; the keys are optional until Config.qml declares them).
    readonly property bool enabled: Config.options?.claude?.bridge?.enable ?? true
    // Send each permission request as a desktop notification (category "permission",
    // actions allow / deny / look), so it shows as a toast and lands in history.
    readonly property bool notifications: Config.options?.claude?.bridge?.notifications ?? true

    // ---- sessions --------------------------------------------------------------------------

    // One entry per session, the ones waiting for you first, then working, then done:
    // {
    //   id:             Claude Code's session id
    //   cwd:            "/home/cakes/src/cakeos"
    //   dir:            "~/src/cakeos"            cwd with the home directory as "~"
    //   name:           "cakeos"                  the session's title when it has one, else the directory's name
    //   state:          "waiting" | "working" | "done" | "idle"
    //                     waiting: needs you (a permission request, or Claude asked something)
    //                     working: a turn is running
    //                     done:    the last turn finished; `summary` says how
    //                     idle:    started or resumed, nothing asked yet
    //   waitingFor:     "permission" | "input" | ""
    //   activity:       "editing shell/end4-pC/modules/ii/bar/Bar.qml" | "running nix build" | ""
    //   tool:           the tool behind `activity` ("Edit", "Bash", ...) or ""
    //   summary:        Claude's last message of the finished turn, plain text, at most 400 characters
    //   error:          "" or why the last turn failed ("rate_limit", "server_error", ...)
    //   message:        text of the last Notification hook ("Claude needs your permission to use Bash")
    //   permissionMode: "default" | "acceptEdits" | "plan" | "auto" | "dontAsk" | "bypassPermissions"
    //   model:          "claude-..." or "" (only known when SessionStart carried it)
    //   startedAt:      ms since the epoch, when the shell first heard of the session
    //   updatedAt:      ms, last event
    //   turnStartedAt:  ms, when the running turn began (0 unless working or waiting)
    //   doneAt:         ms, when the last turn finished (0 until one has)
    //   pid:            the claude process (0 when unknown)
    //   terminal:       { name: "kitty", pid: 4242, windowId: "7", tty: "/dev/pts/3" }, fields empty or 0 when unknown
    //   subagents:      how many subagents are running
    //   ended:          true once the session has exited; it stays listed while it has a result to show
    //   requests:       ids of its pending permission requests
    // }
    property var sessions: []

    readonly property var waiting: sessions.filter(s => s.state === "waiting")
    readonly property var working: sessions.filter(s => s.state === "working")
    readonly property var done: sessions.filter(s => s.state === "done")
    readonly property int waitingCount: waiting.length
    readonly property int workingCount: working.length
    readonly property int doneCount: done.length

    // ---- permission requests ---------------------------------------------------------------

    // Pending requests, oldest first:
    // {
    //   id:          request id
    //   sessionId:   the session that asks
    //   cwd, dir, name: as on the session
    //   tool:        "Bash" | "Edit" | "Write" | "WebFetch" | "mcp__server__tool" | ...
    //   verb:        "run" | "edit" | "write" | "read" | "fetch" | "search" | "use"
    //   subject:     "nix build" | "Bar.qml" | "example.org" | "github: create_issue"   (short)
    //   title:       "~/src/cakeos wants to run nix build"          (dir + verb + subject)
    //   detail:      the whole command, path or URL, at most 2000 characters (the code box)
    //   input:       the tool's input as Claude Code sent it (long strings cut)
    //   agentType:   the subagent that asks, "" for the main agent
    //   createdAt:   ms since the epoch
    //   canAlways:   Claude Code offered a rule to stop asking; allowAlways() applies it
    //   alwaysLabel: what that rule covers: "Bash(nix build:*) in this repo" or ""
    // }
    property var requests: []
    readonly property int requestCount: requests.length

    // ---- signals ---------------------------------------------------------------------------

    signal permissionRequested(var request)
    // outcome: "allow" | "allowAlways" | "deny" (answered from the shell), "terminal" (answered
    // or cancelled in the terminal, or its hook went away), "handedBack", "expired"
    signal permissionResolved(string requestId, string outcome)
    signal sessionStarted(var session)
    // a turn finished: session.summary and session.error are set
    signal sessionDone(var session)
    signal sessionEnded(string sessionId)
    // a Notification hook: type is Claude Code's notification_type ("permission_prompt",
    // "idle_prompt", "elicitation_dialog", ...)
    signal notified(var session, string type, string message)

    // ---- answers ---------------------------------------------------------------------------

    // Allow this one call.
    function allow(requestId: string): void {
        root.send({ type: "decide", id: requestId, behavior: "allow", scope: "once" });
    }

    // Allow it and stop asking for calls like it: applies the rule Claude Code itself
    // suggested for this request (never one made up here). Falls back to allow() when
    // `canAlways` is false.
    function allowAlways(requestId: string): void {
        root.send({ type: "decide", id: requestId, behavior: "allow", scope: "always" });
    }

    // Deny it. `message` (optional) tells Claude why.
    function deny(requestId: string, message: string): void {
        root.send({ type: "decide", id: requestId, behavior: "deny", message: message ?? "" });
    }

    // Give no answer: the prompt in the terminal stands and the request leaves the list.
    function handBack(requestId: string): void {
        root.send({ type: "pass", id: requestId });
    }

    // Drop a done or ended session from the list.
    function dismiss(sessionId: string): void {
        root.send({ type: "dismiss", session: sessionId });
    }

    // Focus the terminal window the session runs in.
    function focus(sessionId: string): void {
        const s = root.sessionById(sessionId);
        if (!s || !(s.terminal?.pid > 0)) return;
        Quickshell.execDetached(["hecate", "msg", "focus", "--pid", String(s.terminal.pid)]);
    }

    function sessionById(id: string): var {
        return root.sessions.find(s => s.id === id) ?? null;
    }

    function requestById(id: string): var {
        return root.requests.find(r => r.id === id) ?? null;
    }

    function requestsFor(sessionId: string): var {
        return root.requests.filter(r => r.sessionId === sessionId);
    }

    // ---- the bridge ------------------------------------------------------------------------

    // Interface stub: the bridge process is wired up in the next commits on this branch.
    function send(message: var): void {
        console.warn("[ClaudeSessions] bridge not wired yet, dropped:", JSON.stringify(message));
    }
}
