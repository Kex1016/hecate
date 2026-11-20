pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "AppleLogic.js" as Logic

// notify: in 10 min. The toast before an event, with a one-line note from
// Claude about what the invite and its attachments say. The note is asked
// for only on a press (`note`); the card says what it would read first.
//
//   AppleEventNote { claude: claudeRunner; event: { title, start, end, calendar, description, attachments: [{name, text}] } }
//
// `event` is one calendar event as the Calendars service lists it.
H.HWindow {
    id: root

    property var claude: null
    property var event: null
    property date now: new Date()
    // pending · thinking · noted · empty · error
    property string phase: "pending"
    property string noteText: ""
    property string message: ""

    readonly property var ev: root.event ? Logic.eventList({ events: [root.event] })[0] ?? null : null
    readonly property int minutes: root.ev ? Math.max(0, Math.round((root.ev.start - root.now) / 60000)) : 0
    readonly property bool readable: root.ev !== null && Logic.noteHasContent(root.ev)

    signal openInvite()
    signal snooze()

    label: root.minutes > 0 ? "notify: in " + root.minutes + " min" : "notify: now"
    variant: "gold"
    implicitWidth: 440
    focus: true
    hints: phase === "pending" && readable ? [{ key: "enter", text: "note" }] : []
    note: phase === "pending" ? "nothing sent yet" : "sent: invite text only"

    function reset(ev) {
        root.event = ev;
        root.noteText = "";
        root.message = "";
        root.phase = "pending";
    }

    function ask() {
        if ((root.phase !== "pending" && root.phase !== "error") || !root.readable || !root.claude)
            return;
        root.message = "";
        root.phase = "thinking";
        root.claude.run(Logic.notePrompt(root.ev));
    }

    Connections {
        target: root.claude
        enabled: root.phase === "thinking"
        function onFinished(ok, text, error) {
            if (!ok) {
                root.message = error;
                root.phase = "error";
                return;
            }
            const n = Logic.cleanNote(text);
            if (n === "") {
                root.phase = "empty";
                return;
            }
            root.noteText = n;
            root.phase = "noted";
        }
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.ask();
            event.accepted = true;
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: root.ev !== null
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
        text: root.ev
            ? "<font color=\"" + H.HTokens.gold + "\">" + (root.ev.allDay ? "all day" : Logic.whenLine({ start: Logic.dateStr(root.ev.start) + "T" + Logic.timeStr(root.ev.start) }).split(" · ").pop()) + "</font> "
              + root.ev.title.replace(/&/g, "&amp;").replace(/</g, "&lt;")
              + (root.ev.calendar ? " <font color=\"" + H.HTokens.muted + "\">· " + ((root.ev.account ? root.ev.account + " · " : "") + root.ev.calendar).replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</font>" : "")
            : ""
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "pending" && root.ev !== null
        text: root.readable ? Logic.noteSummary(root.ev) : "no invite text to read"
        role: "caption"
        ink: "inkMuted"
        wrapMode: Text.WordWrap
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "thinking"
        text: "reading the invite…"
        ink: "muted"
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "noted"
        text: root.noteText
        ink: "rose"
        wrapMode: Text.WordWrap
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "empty"
        text: "nothing in the invite needs doing first"
        ink: "muted"
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.message !== ""
        text: root.message
        ink: "love"
        wrapMode: Text.WordWrap
    }

    RowLayout {
        Layout.topMargin: 8
        spacing: 8
        H.InkButton {
            visible: root.phase === "pending" || root.phase === "error"
            enabled: root.readable && root.claude !== null
            text: root.phase === "error" ? "try again" : "note"
            ink: "c3"
            onClicked: root.ask()
        }
        H.InkButton { text: "open invite"; ink: "c1"; onClicked: root.openInvite() }
        H.InkButton { text: "snooze"; ink: "c1"; onClicked: root.snooze() }
    }
}
