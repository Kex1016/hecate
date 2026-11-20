pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.services

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
    // Why the bridge is not listening, "" when it is or was never tried.
    property string error: ""
    // What Claude Code's hook entries run: `python3 <hookScript>` (the cakeos option writes them).
    readonly property string hookScript: Quickshell.shellPath("scripts/claude/hook.py")
    readonly property string bridgeScript: Quickshell.shellPath("scripts/claude/bridge.py")

    // The bridge runs only while Claude is switched on in the shell (claude.enable, off by
    // default) and claude.bridge.enable is not switched off (on by default; that key and
    // the next are optional until Config.qml declares them).
    readonly property bool enabled: (Config.options?.claude?.enable ?? false) && (Config.options?.claude?.bridge?.enable ?? true)
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
    //                     idle:    at its prompt with nothing to show: just started, or the
    //                              last turn was refused or interrupted
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
    //   terminal:       { name: "kitty", pid: 4242, windowId: "7", tty: "/dev/pts/3" }, fields empty or 0 when unknown.
    //                   windowId is the compositor's id of the terminal's window (WM.windowList), known only
    //                   when the terminal's process has exactly one window; "" otherwise, then use pid
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
    //   canAlways:   Claude Code offered an allow rule to stop asking; allowAlways() applies it.
    //                False when it offered none, or only a mode change or a directory grant
    //   alwaysLabel: what that rule covers: "Bash(nix build:*) in this repo",
    //                "Read(~/notes/**) for this session", or ""
    // }
    property var requests: []
    readonly property int requestCount: requests.length

    // ---- signals ---------------------------------------------------------------------------

    signal permissionRequested(var request)
    // outcome: "allow" | "allowAlways" | "deny" (answered from the shell), "terminal" (answered
    // or cancelled in the terminal, or its hook went away), "handedBack" (by handBack(), or
    // the bridge stopped), "expired" (the hook's wait ran out: `claude -p` waits two minutes)
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

    // Deny it. `message` (optional) tells Claude why. `stop` (optional, true) also ends
    // Claude's turn, as Esc does in the terminal.
    function deny(requestId, message, stop) {
        const decision = { type: "decide", id: String(requestId), behavior: "deny" };
        if (typeof message === "string" && message !== "") decision.message = message;
        if (stop === true) decision.interrupt = true;
        root.send(decision);
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
        if (!s) return;
        if ((s.terminal?.windowId ?? "") !== "")
            Quickshell.execDetached(["hecate", "msg", "focus", "--id", String(s.terminal.windowId)]);
        else if (s.terminal?.pid > 0)
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

    // The sessions as the bridge reports them; `sessions` adds each terminal's window.
    property var bridgeSessions: []
    property int restartDelay: 1000

    // One line to the bridge. Dropped while the bridge is not listening: an answer is never
    // kept for later.
    function send(message: var): void {
        if (!root.available || !bridge.running) {
            console.warn("[ClaudeSessions] the bridge is not running, dropped:", message?.type);
            return;
        }
        bridge.write(JSON.stringify(message) + "\n");
    }

    // One line from the bridge: its state first, then the events that led to it.
    function receive(line: string): void {
        let message = null;
        try {
            message = JSON.parse(line);
        } catch (e) {
            return;
        }
        if (message === null || typeof message !== "object") return;
        switch (message.type) {
        case "state":
            root.bridgeSessions = Array.isArray(message.sessions) ? message.sessions : [];
            root.requests = Array.isArray(message.requests) ? message.requests : [];
            root.sessions = root.bridgeSessions.map(s => root.withWindow(s));
            break;
        case "event":
            root.dispatch(message);
            break;
        case "hello":
            root.socketPath = message.socket ?? "";
            root.error = "";
            root.available = true;
            settled.restart();
            root.send({ type: "config", notifications: root.notifications });
            break;
        case "error":
            root.error = message.message ?? "the bridge failed";
            console.warn("[ClaudeSessions] bridge:", root.error);
            break;
        case "log":
            console.info("[ClaudeSessions] bridge:", message.message ?? "");
            break;
        }
    }

    function dispatch(event: var): void {
        switch (event.name) {
        case "permissionRequested":
            if (event.request) root.permissionRequested(event.request);
            break;
        case "permissionResolved":
            root.permissionResolved(event.id ?? "", event.outcome ?? "");
            break;
        case "sessionStarted":
            if (event.session) root.sessionStarted(root.withWindow(event.session));
            break;
        case "sessionDone":
            if (event.session) root.sessionDone(root.withWindow(event.session));
            break;
        case "sessionEnded":
            root.sessionEnded(event.id ?? "");
            break;
        case "notified":
            if (event.session) root.notified(root.withWindow(event.session), event.notificationType ?? "", event.message ?? "");
            break;
        case "look":
            // the "look" action of a request's notification
            root.focus(event.session ?? "");
            break;
        }
    }

    // The session with terminal.windowId filled in, when its terminal's process has one
    // window. (A terminal that keeps several windows in one process cannot be told apart.)
    function withWindow(session: var): var {
        const pid = session?.terminal?.pid ?? 0;
        if (!(pid > 0)) return session;
        const own = (WM.windowList ?? []).filter(w => w.pid === pid);
        if (own.length !== 1) return session;
        return Object.assign({}, session, { terminal: Object.assign({}, session.terminal, { windowId: String(own[0].id) }) });
    }

    Connections {
        target: WM
        function onWindowListChanged() {
            if (root.bridgeSessions.length === 0) return;
            const next = root.bridgeSessions.map(s => root.withWindow(s));
            if (next.some((s, i) => s.terminal?.windowId !== root.sessions[i]?.terminal?.windowId))
                root.sessions = next;
        }
    }

    // The bridge stopped. It handed every waiting hook back to its terminal on the way out,
    // and a hook that runs while it is down finds no socket and does the same.
    function stopped(): void {
        const pending = root.requests;
        root.available = false;
        root.socketPath = "";
        root.bridgeSessions = [];
        root.sessions = [];
        root.requests = [];
        for (const request of pending)
            root.permissionResolved(request.id, "handedBack");
        settled.stop();
        if (!root.wanted) return;
        restart.interval = root.restartDelay;
        restart.restart();
        root.restartDelay = Math.min(root.restartDelay * 2, 30000);
    }

    readonly property bool wanted: root.enabled && Config.ready
    onWantedChanged: {
        restart.stop();
        root.restartDelay = 1000;
        bridge.running = root.wanted;
    }
    onNotificationsChanged: {
        if (root.available) root.send({ type: "config", notifications: root.notifications });
    }

    Process {
        id: bridge
        command: ["python3", root.bridgeScript]
        running: false
        stdinEnabled: true
        stdout: SplitParser {
            onRead: line => root.receive(line)
        }
        onExited: (exitCode, exitStatus) => root.stopped()
    }

    Timer {
        id: restart
        onTriggered: {
            if (root.wanted) bridge.running = true;
        }
    }

    // a bridge that has stayed up starts again quickly the next time
    Timer {
        id: settled
        interval: 30000
        onTriggered: root.restartDelay = 1000
    }

    Component.onCompleted: bridge.running = root.wanted
}
