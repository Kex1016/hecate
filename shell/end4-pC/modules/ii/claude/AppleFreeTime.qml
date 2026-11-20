pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "AppleLogic.js" as Logic

// claude: free time. "when am I free this week" is answered from the
// calendars you show. The card lists what it will read before the press
// (calendar names, how many events, which days); only event titles, times
// and calendar names are sent, never notes, places or guests.
//
//   AppleFreeTime { claude: claudeRunner; calendars: Calendars; question: "when am I free this week" }
H.HWindow {
    id: root

    property var claude: null
    property var calendars: null
    property string question: ""
    property date now: new Date()
    // pending · thinking · answered · error
    property string phase: "pending"
    property string answer: ""
    property string message: ""

    readonly property var ctx: Logic.freeContext(Logic.eventList(root.calendars), root.now)

    signal dismissed()

    label: "claude: free time"
    variant: phase === "error" ? "love" : "pine"
    implicitWidth: 480
    focus: true
    hints: phase === "pending" ? [{ key: "enter", text: "ask" }, { key: "esc", text: "close" }] : [{ key: "esc", text: "close" }]
    note: phase === "pending" ? "nothing sent yet" : "sent: titles and times only"

    function reset(text) {
        root.question = text ?? "";
        root.answer = "";
        root.message = "";
        root.phase = "pending";
        root.forceActiveFocus();
    }

    function ask() {
        if ((root.phase !== "pending" && root.phase !== "error") || root.question.trim() === "" || !root.claude)
            return;
        root.message = "";
        root.phase = "thinking";
        root.claude.run(Logic.freePrompt(root.question, root.ctx, root.now));
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
            const t = text.replace(/[`*#]/g, "").trim();
            if (t === "") {
                root.message = "claude had nothing to say";
                root.phase = "error";
                return;
            }
            root.answer = t;
            root.phase = "answered";
        }
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.ask();
            event.accepted = true;
        } else if (event.key === Qt.Key_Escape) {
            root.dismissed();
            event.accepted = true;
        }
    }

    H.HText {
        Layout.fillWidth: true
        text: "<font color=\"" + H.HTokens.gold + "\">?</font> " + root.question.replace(/&/g, "&amp;").replace(/</g, "&lt;")
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
    }

    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 6
        visible: root.phase === "pending" || root.phase === "thinking"
        H.HLabel { text: "reading"; height: 22; verticalAlignment: Text.AlignVCenter }
        Repeater {
            model: root.ctx.calendarNames
            H.Chip { required property string modelData; text: modelData; on: true }
        }
        H.Chip { text: root.ctx.events.length + " events"; on: true }
        H.Chip { text: root.ctx.summary.split(" · ").pop(); on: true }
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 6
        visible: root.phase === "thinking"
        text: "asking claude…"
        ink: "muted"
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "answered"
        text: root.answer
        wrapMode: Text.WordWrap
    }

    H.HText {
        Layout.fillWidth: true
        visible: root.phase === "answered"
        text: root.ctx.summary
        role: "caption"
        ink: "inkMuted"
        wrapMode: Text.WordWrap
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
        visible: root.phase === "pending" || root.phase === "error"
        H.InkButton {
            text: root.phase === "error" ? "try again" : "ask claude"
            ink: "c3"
            enabled: root.question.trim() !== "" && root.claude !== null
            onClicked: root.ask()
        }
        H.InkButton { text: "close"; ink: "c1"; onClicked: root.dismissed() }
    }
}
