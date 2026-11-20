pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "SessionsFormat.js" as F

// The sessions panel (Mod+Shift+/): the haiiro window `claude: ~/sessions`
// with this window's usage and one card per Claude Code session, waiting
// ones first. `a` and `d` answer the selected waiting session, enter goes to
// its terminal, the arrows move the selection.
//
// `sessions` is the list as ClaudeSessions reports it. The panel never
// answers on its own: every press leaves as a signal, and Sessions.qml sends
// it back to the session.
H.HWindow {
    id: root

    property var sessions: []
    property real usagePercent: 0
    property string usageResets: ""
    property bool usageKnown: false
    // how a session's terminal reads on the card ("kitty · ws 3")
    property var placeOf: s => ""

    signal answer(var session, string decision)
    signal goToTerminal(var session)
    signal showDiff(var session)
    signal dismissSession(var session)
    signal closeRequested()

    readonly property var ordered: F.order(sessions)
    property string selectedId: ""
    readonly property var selected: ordered.find(s => s.id === selectedId) ?? ordered[0] ?? null
    readonly property string requestId: selected?.request?.id ?? ""
    // Answers by key wait a moment after the request on screen changes, so a
    // keystroke meant for one command cannot answer the next one.
    property bool armed: true
    onRequestIdChanged: { armed = false; arm.restart(); }
    Timer { id: arm; interval: 600; onTriggered: root.armed = true }

    property double now: Date.now()
    Timer {
        running: true
        interval: 5000
        repeat: true
        onTriggered: root.now = Date.now()
    }

    function move(delta) {
        if (ordered.length === 0) return;
        const i = Math.max(0, ordered.findIndex(s => s.id === (selected?.id ?? "")));
        selectedId = ordered[(i + delta + ordered.length) % ordered.length].id;
    }

    label: "claude: ~/sessions"
    hints: [
        { key: "enter", text: "go to session" },
        { key: "a", text: "allow" },
        { key: "d", text: "deny" }
    ]
    note: "Mod+Shift+/ closes"
    implicitWidth: 620
    focus: true

    Shortcut { sequence: "Up"; enabled: root.visible; onActivated: root.move(-1) }
    Shortcut { sequence: "Down"; enabled: root.visible; onActivated: root.move(1) }

    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return;
        const s = root.selected;
        if (event.key === Qt.Key_Escape) root.closeRequested();
        else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { if (s) root.goToTerminal(s); }
        else if (event.key === Qt.Key_A && s?.state === "waiting") { if (root.armed) root.answer(s, "allow"); }
        else if (event.key === Qt.Key_D && s?.state === "waiting") { if (root.armed) root.answer(s, "deny"); }
        else return;
        event.accepted = true;
    }

    // ---- usage ----
    ColumnLayout {
        visible: root.usageKnown
        Layout.fillWidth: true
        Layout.bottomMargin: 12
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            H.HText { text: "usage" }
            H.HText { Layout.leftMargin: 8; text: "this window"; role: "caption"; ink: "c1" }
            Item { Layout.fillWidth: true }
            H.HText {
                text: Math.round(root.usagePercent * 100) + "%" + (root.usageResets ? " · resets " + root.usageResets : "")
                role: "code"
                ink: "inkMuted"
            }
        }
        H.Progress {
            Layout.fillWidth: true
            value: root.usagePercent
        }
    }

    // ---- nothing running ----
    H.HText {
        visible: root.ordered.length === 0
        Layout.fillWidth: true
        text: "No Claude Code sessions are running."
    }
    H.HText {
        visible: root.ordered.length === 0
        Layout.fillWidth: true
        Layout.topMargin: 8
        Layout.bottomMargin: 4
        text: "Start claude in a terminal and it reports here; a request to run something waits here and in a toast until you answer it."
        ink: "c1"
        wrapMode: Text.WordWrap
    }

    // ---- one card per session ----
    Repeater {
        model: root.ordered
        SessionsCard {
            id: card
            required property var modelData
            required property int index
            Layout.fillWidth: true
            Layout.bottomMargin: index === root.ordered.length - 1 ? 0 : 12
            session: modelData
            selected: root.selected?.id === modelData.id
            now: root.now
            place: root.placeOf(modelData)
            onAllow: root.answer(modelData, "allow")
            onAlways: root.answer(modelData, "always")
            onDeny: root.answer(modelData, "deny")
            onGoToTerminal: root.goToTerminal(modelData)
            onShowDiff: root.showDiff(modelData)
            onDismiss: root.dismissSession(modelData)
            TapHandler {
                // a click anywhere else on the card selects it; the buttons keep theirs
                gesturePolicy: TapHandler.ReleaseWithinBounds
                onTapped: root.selectedId = card.modelData.id
            }
        }
    }
}
