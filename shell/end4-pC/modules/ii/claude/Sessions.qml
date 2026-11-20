pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.services.claude
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// Claude sessions: the panel on Mod+Shift+/ (the `claudeSessionsToggle`
// shortcut), the permission and done toasts on their own layer, and the gold
// border on the window of a session that waits for you.
//
// All three read one source, ClaudeSessions (services/claude), and answer
// through it. Nothing here answers a request by itself: allow, deny and
// "always in this repo" leave only from a press the person made.
Scope {
    id: root

    // ClaudeSessions (services/claude): the sessions, the pending permission requests, and
    // the way to answer them. CLAUDE_SESSIONS_MOCK=1 runs this on SessionsMock instead, for
    // looking at the panel without a session running.
    property var source: Quickshell.env("CLAUDE_SESSIONS_MOCK") === "1" ? mock : ClaudeSessions

    SessionsMock { id: mock }

    readonly property var requests: source?.requests ?? []
    // each session with the request it waits on (the oldest), when it has one; ended
    // sessions stay only while they have a result to show
    readonly property var sessions: (source?.sessions ?? [])
        .filter(s => !s.ended || s.state === "done")
        .map(s => Object.assign({}, s, { request: root.requests.find(r => r.sessionId === s.id) ?? null }))
    readonly property var waiting: sessions.filter(s => s.state === "waiting")

    // where the panel sits on the board: 60px from the left, 30px under the bar
    readonly property int marginLeft: 60
    readonly property int marginTop: H.HTokens.barHeight + 30

    readonly property string screenName: WM.focusedMonitor?.name ?? ""

    // the bar's badge, and what a fullscreen window hides the toasts behind
    onWaitingChanged: SessionsState.waiting = waiting.length
    Component.onCompleted: SessionsState.waiting = waiting.length

    // ---- what a press does ----

    // The compositor window of a session's terminal, when the hook knew its id and it is
    // still the same program. Otherwise the terminal is only known by its process.
    function windowFor(session) {
        const id = session?.terminal?.windowId ?? "";
        if (id === "" || id === "0") return null;
        const win = WM.windowList.find(w => String(w.id) === String(id));
        const name = String(session.terminal.name ?? "").toLowerCase();
        if (!win || (name !== "" && !String(win.appId ?? "").toLowerCase().includes(name))) return null;
        return win;
    }

    // "kitty · ws 3"
    function placeOf(session) {
        const name = session?.terminal?.name ?? "";
        const win = root.windowFor(session);
        const ws = win ? WM.workspaceById[win.workspaceId] : null;
        return win ? name + " · ws " + (ws?.idx ?? win.workspaceId) : name;
    }

    // A press answers the request that was on screen when it happened, and only while that
    // request is still pending for that session: a session that has moved on to another
    // request, or none, is answered by nothing.
    function answerSession(sessionId, requestId, decision) {
        const r = root.requests.find(x => x.id === requestId);
        if (!root.source || !r || r.sessionId !== sessionId) return;
        if (decision === "allow") root.source.allow(requestId);
        else if (decision === "always" && r.canAlways === true) root.source.allowAlways(requestId);
        else if (decision === "deny") root.source.deny(requestId, "");
    }

    function goToTerminal(session) {
        const win = root.windowFor(session);
        if (win) {
            WM.switchWorkspace(win.workspaceId);
            WM.focusWindow(win.id);
        } else if ((session?.terminal?.pid ?? 0) > 0) {
            root.source.focus(session.id);
        } else {
            return false;
        }
        SessionsState.open = false;
        return true;
    }

    function showDiff(session) {
        if (!session?.cwd) return;
        const terminal = Config.options.apps.terminal;
        Quickshell.execDetached({
            command: ["sh", "-c", terminal + " sh -c 'git diff --color=always | less -R'"],
            workingDirectory: session.cwd
        });
        SessionsState.open = false;
    }

    function dismissSession(session) {
        root.source?.dismiss(session.id);
    }

    // the terminals of the sessions that wait, as the tint finds them
    function tintTargets() {
        const out = [];
        for (const s of root.waiting) {
            const win = root.windowFor(s);
            if (win) out.push("id:" + win.id);
            else if ((s.terminal?.pid ?? 0) > 0) out.push("pid:" + s.terminal.pid);
        }
        return out;
    }

    // ---- the panel ----

    // the surface stays mapped until the close frames have played
    property bool keepAlive: false
    Connections {
        target: SessionsState
        function onOpenChanged() {
            if (SessionsState.open) root.keepAlive = true;
        }
    }

    Loader {
        id: panelLoader
        active: SessionsState.open || root.keepAlive

        sourceComponent: PanelWindow {
            id: panelWindow
            visible: true

            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0
            readonly property int pad: transition.margin
            implicitWidth: transition.width + 2 * pad
            implicitHeight: transition.height + 2 * pad
            color: "transparent"
            WlrLayershell.namespace: "quickshell:claudeSessions"
            // a, d, enter and the arrows need the keyboard
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand

            anchors {
                top: true
                left: true
            }
            margins {
                top: root.marginTop - panelWindow.pad
                left: root.marginLeft - panelWindow.pad
            }

            mask: Region {
                item: transition
            }

            Component.onCompleted: GlobalFocusGrab.addDismissable(panelWindow)
            Component.onDestruction: GlobalFocusGrab.removeDismissable(panelWindow)
            Connections {
                target: GlobalFocusGrab
                function onDismissed() {
                    SessionsState.open = false;
                }
            }

            H.PixelTransition {
                id: transition
                x: panelWindow.pad
                y: panelWindow.pad
                shown: SessionsState.open
                width: panel.implicitWidth
                height: panel.implicitHeight
                onClosed: root.keepAlive = false

                SessionsPanel {
                    id: panel
                    width: transition.width
                    height: transition.height
                    sessions: root.sessions
                    usagePercent: root.source?.usagePercent ?? 0
                    usageResets: root.source?.usageResets ?? ""
                    usageKnown: root.source?.usageKnown ?? false
                    placeOf: s => root.placeOf(s)
                    onAnswer: (session, decision) => root.answerSession(session.id, session.request?.id ?? "", decision)
                    onGoToTerminal: session => root.goToTerminal(session)
                    onShowDiff: session => root.showDiff(session)
                    onDismissSession: session => root.dismissSession(session)
                    onCloseRequested: SessionsState.open = false
                    Component.onCompleted: forceActiveFocus()
                }
            }
        }
    }

    // ---- the toasts, on their own layer ----

    SessionsToasts {
        sessions: root.sessions
        requests: root.requests
        silent: Notifications.silent
        fullscreen: WM.fullscreenOnMonitor(root.screenName)
        locked: GlobalStates.screenLocked
        screenName: root.screenName
        onAnswer: (sessionId, requestId, decision) => root.answerSession(sessionId, requestId, decision)
        onLook: sessionId => {
            const s = root.sessions.find(x => x.id === sessionId);
            if (!s || !root.goToTerminal(s)) SessionsState.open = true;
        }
    }

    // ---- the gold border ----

    SessionsTint {
        enabled: WM.compositor === "hecate"
        targets: root.tintTargets()
    }

    // ---- ways in ----

    IpcHandler {
        target: "claudeSessions"

        function toggle(): void {
            SessionsState.open = !SessionsState.open;
        }

        function open(): void {
            SessionsState.open = true;
        }

        function close(): void {
            SessionsState.open = false;
        }
    }

    CompositorGlobalShortcut {
        name: "claudeSessionsToggle"
        description: "Toggles the Claude sessions panel on press"

        onPressed: {
            SessionsState.open = !SessionsState.open;
        }
    }
}
