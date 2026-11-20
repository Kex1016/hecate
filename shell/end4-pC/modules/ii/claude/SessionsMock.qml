import QtQuick
import Quickshell

// A stand-in for services/claude/ClaudeSessions.qml with the same fields and
// calls, used to render the panel headless and to try it without a session
// running (CLAUDE_SESSIONS_MOCK=1). Answering here only changes this list;
// nothing is sent anywhere.
QtObject {
    id: root

    readonly property string home: Quickshell.env("HOME") ?? "/home/user"
    property var answered: []
    property var focused: []

    property var sessions: {
        const now = Date.now();
        return [
            {
                id: "s1", cwd: root.home + "/src/cakeos", dir: "~/src/cakeos", name: "cakeos",
                state: "waiting", waitingFor: "permission", activity: "", tool: "Bash", summary: "", error: "",
                message: "Claude needs your permission to use Bash", startedAt: now - 9 * 60000,
                updatedAt: now - 5000, turnStartedAt: now - 60000, doneAt: 0, ended: false,
                terminal: { name: "kitty", pid: 4242, windowId: "4", tty: "/dev/pts/3" }, requests: ["r1"]
            },
            {
                id: "s2", cwd: root.home + "/src/wl-hex", dir: "~/src/wl-hex", name: "wl-hex",
                state: "working", waitingFor: "", activity: "editing shell/end4-pC/modules/ii/bar/Bar.qml", tool: "Edit",
                summary: "", error: "", message: "", startedAt: now - 20 * 60000, updatedAt: now - 2000,
                turnStartedAt: now - 12 * 60000, doneAt: 0, ended: false,
                terminal: { name: "kitty", pid: 4300, windowId: "7", tty: "/dev/pts/4" }, requests: []
            },
            {
                id: "s3", cwd: root.home + "/src/hecate-greetd", dir: "~/src/hecate-greetd", name: "hecate-greetd",
                state: "done", waitingFor: "", activity: "", tool: "",
                summary: "Kept the GPU on through the handover; cargo test passes. 2 files changed, not committed.",
                error: "", message: "", startedAt: now - 30 * 60000, updatedAt: now - 3 * 60000,
                turnStartedAt: 0, doneAt: now - 3 * 60000, ended: false,
                terminal: { name: "kitty", pid: 4400, windowId: "", tty: "" }, requests: []
            }
        ];
    }

    property var requests: [{
        id: "r1", sessionId: "s1", cwd: root.home + "/src/cakeos", dir: "~/src/cakeos", name: "cakeos",
        tool: "Bash", verb: "run", subject: "nix build",
        title: "~/src/cakeos wants to run nix build",
        detail: "nix build .#nixosConfigurations.cakeos.config.system.build.toplevel",
        input: {}, agentType: "", createdAt: Date.now() - 5000,
        canAlways: true, alwaysLabel: "Bash(nix build:*) in this repo"
    }]

    // a fourth session finishes a few seconds after start, for the done toast
    property Timer finisher: Timer {
        interval: Number(Quickshell.env("CLAUDE_SESSIONS_MOCK_FINISH_MS") ?? 5000)
        running: true
        onTriggered: root.sessions = root.sessions.concat([{
            id: "s4", cwd: root.home + "/src/cakeos-wm", dir: "~/src/cakeos-wm", name: "cakeos-wm",
            state: "done", waitingFor: "", activity: "", tool: "",
            summary: "Switched the desktop and restored its state; 2 files changed · tests pass",
            error: "", message: "", startedAt: Date.now() - 4 * 60000, updatedAt: Date.now(),
            turnStartedAt: 0, doneAt: Date.now(), ended: false,
            terminal: { name: "kitty", pid: 4500, windowId: "9", tty: "" }, requests: []
        }])
    }

    function resolve(requestId, how) {
        root.answered = root.answered.concat([requestId + ":" + how]);
        const r = root.requests.find(x => x.id === requestId);
        root.requests = root.requests.filter(x => x.id !== requestId);
        root.sessions = root.sessions.map(s => r && s.id === r.sessionId
            ? Object.assign({}, s, { state: "working", waitingFor: "", activity: "running " + r.tool, requests: [] }) : s);
    }

    function allow(requestId) { root.resolve(requestId, "allow"); }
    function allowAlways(requestId) { root.resolve(requestId, "always"); }
    function deny(requestId, message) { root.resolve(requestId, "deny"); }
    function dismiss(sessionId) { root.sessions = root.sessions.filter(s => s.id !== sessionId); }
    function focus(sessionId) { root.focused = root.focused.concat([sessionId]); }
}
