pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H
import "SessionsFormat.js" as F

// Claude's own toast layer. Permission requests and done notices are drawn
// here, from the session list, and not through the notification service, so
// do-not-disturb has no say over a request that is waiting for you:
//
//   permission toasts  always shown, and stay until answered. Nothing answers
//                      them for you: no timeout, no default.
//   done toasts        follow do-not-disturb, go after 8 seconds or a click,
//                      and are only shown for a session that finished just now
//
// A fullscreen window on this monitor hides the layer (the bar's badge counts
// what waits); the requests stay where they are and show again afterwards.
Scope {
    id: root

    // the lists and the way back to the sessions, from Sessions.qml
    property var sessions: []
    property var requests: []
    // do-not-disturb, held back from here for done notices only
    property bool silent: false
    property bool fullscreen: false
    property bool locked: false
    property string screenName: ""

    signal answer(string sessionId, string requestId, string decision)
    signal look(string sessionId)

    readonly property int maxShown: 3
    readonly property int pad: 12 // room for the pixelate margin around each window
    readonly property int doneMs: 8000
    // a session counts as just finished for this long after it says so
    readonly property int freshMs: 15000

    // done notices already considered, so a session is toasted once per finish
    property var seen: ({})

    ListModel { id: entries }

    function indexOfKey(key) {
        for (let i = 0; i < entries.count; i++)
            if (entries.get(i).key === key) return i;
        return -1;
    }

    function sync() {
        const now = Date.now();
        const live = {};
        for (const r of root.requests) {
            const key = "p:" + r.id;
            live[key] = true;
            if (indexOfKey(key) === -1) {
                entries.insert(0, {
                    key: key, kind: "permission", shown: true, sessionId: r.sessionId, requestId: r.id,
                    repo: r.dir || r.cwd || "", verb: r.verb || "use", subject: r.subject || r.tool || "",
                    command: r.detail || "", summary: "", error: "", canAlways: r.canAlways === true, expireAt: 0
                });
            }
        }
        for (const s of root.sessions) {
            if (s.state !== "done" || !(s.doneAt > 0)) continue;
            const key = "d:" + s.id + ":" + s.doneAt;
            live[key] = true;
            if (root.seen[key] !== undefined) continue;
            root.seen[key] = true;
            if (root.silent || now - s.doneAt >= root.freshMs) continue;
            entries.append({
                key: key, kind: "done", shown: true, sessionId: s.id, requestId: "",
                repo: s.dir || s.cwd || "", verb: "", subject: "", command: "",
                summary: F.firstLine(s.summary, 90), error: s.error || "", canAlways: false, expireAt: now + root.doneMs
            });
        }
        // what is no longer true (answered, dismissed, started again) closes
        for (let i = 0; i < entries.count; i++)
            if (!live[entries.get(i).key]) entries.setProperty(i, "shown", false);
        // permission first
        for (let i = 1; i < entries.count; i++)
            if (entries.get(i).kind === "permission" && entries.get(i - 1).kind !== "permission") entries.move(i, i - 1, 1);
        root.pruneSeen(live);
    }

    function pruneSeen(live) {
        for (const k of Object.keys(root.seen))
            if (!live[k]) delete root.seen[k];
    }

    function close(key) {
        const i = indexOfKey(key);
        if (i !== -1) entries.setProperty(i, "shown", false);
    }

    function drop(key) {
        const i = indexOfKey(key);
        if (i !== -1 && !entries.get(i).shown) entries.remove(i);
    }

    onSessionsChanged: sync()
    onRequestsChanged: sync()
    Component.onCompleted: sync()

    // done notices end themselves
    Timer {
        interval: 500
        repeat: true
        running: entries.count > 0
        onTriggered: {
            const now = Date.now();
            for (let i = 0; i < entries.count; i++) {
                const e = entries.get(i);
                if (e.shown && e.kind === "done" && e.expireAt > 0 && now >= e.expireAt)
                    entries.setProperty(i, "shown", false);
            }
        }
    }

    // do-not-disturb pulls a done notice that has not been dismissed yet
    onSilentChanged: {
        if (!silent) return;
        for (let i = 0; i < entries.count; i++)
            if (entries.get(i).kind === "done") entries.setProperty(i, "shown", false);
    }

    PanelWindow {
        id: window
        visible: entries.count > 0 && !root.fullscreen && !root.locked
        screen: Quickshell.screens.find(s => s.name === root.screenName) ?? null

        WlrLayershell.namespace: "quickshell:claudeToasts"
        WlrLayershell.layer: WlrLayer.Overlay
        exclusiveZone: 0

        anchors {
            top: true
            right: true
        }
        margins {
            top: H.HTokens.barHeight + 32 - root.pad
            right: 24 - root.pad
        }

        color: "transparent"
        implicitWidth: stack.width + 2 * root.pad
        implicitHeight: stack.height + 2 * root.pad
        mask: Region {
            item: stack
        }

        Column {
            id: stack
            x: root.pad
            y: root.pad
            width: H.HTokens.toastWidth
            spacing: H.HTokens.toastGap

            Repeater {
                model: entries
                SessionsToast {
                    id: toast
                    required property var model
                    shown: model.shown
                    kind: model.kind
                    repo: model.repo
                    verb: model.verb
                    subject: model.subject
                    command: model.command
                    summary: model.summary
                    error: model.error
                    canAlways: model.canAlways
                    onAllow: root.answer(model.sessionId, model.requestId, "allow")
                    onAlways: root.answer(model.sessionId, model.requestId, "always")
                    onDeny: root.answer(model.sessionId, model.requestId, "deny")
                    onLook: root.look(model.sessionId)
                    onDismissed: root.close(model.key)
                    onClosed: root.drop(model.key)
                }
            }
        }
    }

    // how far down the notification stack has to start to clear this one
    Binding {
        target: SessionsState
        property: "toastsHeight"
        value: window.visible ? stack.height + H.HTokens.toastGap : 0
    }
}
