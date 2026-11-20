pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "AppleLogic.js" as Logic

// claude: new event. Plain words ("dinner with kuro sat 8") become a card
// the user confirms; only the `add` press hands the event to the calendar
// service. Nothing leaves the shell before the first press (`ask`).
//
//   AppleEventCard { claude: claudeRunner; calendars: Calendars; request: "dinner with kuro sat 8" }
//
// `calendars` is the W2-cal Calendars service: `targets` ("local" or
// "account: calendar" strings) and createEvent(ev, (error, event) => ...).
// The service has no guest list, so "with Kuro" goes into the event's notes.
H.HWindow {
    id: root

    property var claude: null
    property var calendars: null
    property string request: ""
    property string defaultCalendar: ""
    property date now: new Date()
    // pending: waiting for the press · thinking · proposed · editing · adding · added · refused · error
    property string phase: "pending"
    property var proposal: null
    property string message: ""

    readonly property var calList: Logic.calendarList(root.calendars)
    readonly property bool busy: phase === "thinking"

    signal created(var event)
    signal dismissed()

    label: "claude: new event"
    variant: phase === "error" ? "love" : (phase === "proposed" || phase === "editing") ? "gold" : phase === "added" ? "foam" : "pine"
    implicitWidth: 480
    focus: true
    hints: phase === "pending" ? [{ key: "enter", text: "ask" }, { key: "esc", text: "close" }]
        : (phase === "proposed" || phase === "editing") ? [{ key: "enter", text: "add" }, { key: "esc", text: "no" }]
        : [{ key: "esc", text: "close" }]
    note: phase === "pending" ? "nothing sent yet" : (claude && claude.sent !== "" ? "sent: your words, calendar names, the time" : "")

    function reset(text) {
        root.request = text ?? "";
        root.proposal = null;
        root.message = "";
        root.phase = "pending";
        root.forceActiveFocus();
    }

    // The press. Builds the prompt from what the chips list and sends it.
    function ask() {
        if (root.phase !== "pending" && root.phase !== "error")
            return;
        if (root.request.trim() === "" || !root.claude)
            return;
        root.message = "";
        root.phase = "thinking";
        root.claude.run(Logic.eventPrompt(root.request, root.now, root.calList));
    }

    function add() {
        const ev = root.proposal;
        if ((root.phase !== "proposed" && root.phase !== "editing") || !ev)
            return;
        const svc = root.calendars;
        if (!svc || typeof svc.createEvent !== "function") {
            root.message = "the calendar service is not available";
            root.phase = "error";
            return;
        }
        if (ev.calendar === "" && root.calList.length > 0) {
            root.message = "pick a calendar first";
            return;
        }
        root.message = "";
        root.phase = "adding";
        svc.createEvent(Logic.createPayload(ev), (error, stored) => {
            if (error && error !== "") {
                root.message = error;
                root.phase = "error";
                return;
            }
            root.phase = "added";
            root.created(ev);
        });
    }

    function no() {
        root.phase = "refused";
        root.proposal = null;
        root.dismissed();
    }

    function edit(field, value) {
        const next = Logic.applyEdit(root.proposal, field, value);
        if (next)
            root.proposal = next;
        return next !== null;
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
            const r = Logic.normalizeEvent(Logic.extractJson(text), root.now, root.calList, root.defaultCalendar);
            if (!r.ok) {
                root.message = r.error;
                root.phase = "error";
                return;
            }
            root.proposal = r.event;
            root.phase = "proposed";
        }
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (root.phase === "pending" || root.phase === "error") root.ask();
            else root.add();
            event.accepted = true;
        } else if (event.key === Qt.Key_Escape) {
            if (root.phase === "proposed" || root.phase === "editing") root.no();
            else root.dismissed();
            event.accepted = true;
        }
    }

    H.PromptLine {
        Layout.fillWidth: true
        path: "~"
        command: "?"
        args: root.request
    }

    // what will be sent, before the press
    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 6
        visible: root.phase === "pending" || root.phase === "thinking"
        H.HLabel { text: "sending"; height: 22; verticalAlignment: Text.AlignVCenter }
        H.Chip { text: "your words"; on: true }
        H.Chip { text: root.calList.length + " calendar names"; on: true }
        H.Chip { text: "the time"; on: true }
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 6
        visible: root.phase === "thinking" || root.phase === "adding"
        text: root.phase === "adding" ? "adding…" : "asking claude…"
        ink: "muted"
    }

    // the proposed event
    Rectangle {
        Layout.fillWidth: true
        Layout.topMargin: 6
        visible: (root.phase === "proposed" || root.phase === "added") && root.proposal !== null
        color: H.HTokens.base
        implicitHeight: lines.implicitHeight + 12

        ColumnLayout {
            id: lines
            x: 10
            y: 6
            width: parent.width - 20
            spacing: 2
            Repeater {
                model: root.proposal ? Logic.eventCardLines(root.proposal) : []
                H.HText {
                    required property string modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData
                    ink: index === 0 ? "gold" : index === 1 ? "text" : "muted"
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    // editing
    ColumnLayout {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 4
        visible: root.phase === "editing" && root.proposal !== null
        AppleField {
            Layout.fillWidth: true
            label: "title"
            value: root.proposal ? root.proposal.title : ""
            onCommitted: text => { invalid = !root.edit("title", text); }
        }
        AppleField {
            Layout.fillWidth: true
            label: "start"
            value: root.proposal ? root.proposal.start : ""
            onCommitted: text => { invalid = !root.edit("start", text); }
        }
        AppleField {
            Layout.fillWidth: true
            label: "end"
            value: root.proposal ? root.proposal.end : ""
            onCommitted: text => { invalid = !root.edit("end", text); }
        }
        AppleField {
            Layout.fillWidth: true
            label: "where"
            value: root.proposal ? root.proposal.location : ""
            onCommitted: text => { invalid = !root.edit("location", text); }
        }
        AppleField {
            Layout.fillWidth: true
            label: "with"
            value: root.proposal ? root.proposal.invite.join(", ") : ""
            onCommitted: text => { invalid = !root.edit("invite", text); }
        }
        Flow {
            Layout.fillWidth: true
            Layout.topMargin: 4
            spacing: 6
            H.HLabel { text: "calendar"; height: 22; verticalAlignment: Text.AlignVCenter }
            Repeater {
                model: root.calList.filter(c => c.writable !== false)
                H.Chip {
                    required property var modelData
                    text: modelData.id
                    interactive: true
                    on: root.proposal !== null && root.proposal.calendar === modelData.id
                    onClicked: {
                        const next = Object.assign({}, root.proposal);
                        next.calendar = modelData.id;
                        next.calendarName = modelData.name;
                        next.account = modelData.account;
                        root.proposal = next;
                    }
                }
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.proposal !== null && root.proposal.past === true && (root.phase === "proposed" || root.phase === "editing")
        text: "that time is already past"
        ink: "love"
        role: "caption"
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "added"
        text: "added to " + (root.proposal ? root.proposal.calendar : "")
        ink: "foam"
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
        visible: root.phase !== "added" && root.phase !== "refused" && root.phase !== "thinking" && root.phase !== "adding"

        H.InkButton {
            visible: root.phase === "pending" || root.phase === "error"
            text: root.phase === "error" ? "try again" : "ask claude"
            ink: "c3"
            enabled: root.request.trim() !== "" && root.claude !== null
            onClicked: root.ask()
        }
        H.InkButton {
            visible: root.phase === "proposed" || root.phase === "editing"
            text: "add"
            ink: "c3"
            onClicked: root.add()
        }
        H.InkButton {
            visible: root.phase === "proposed"
            text: "edit"
            ink: "c1"
            onClicked: root.phase = "editing"
        }
        H.InkButton {
            visible: root.phase === "editing"
            text: "done"
            ink: "c1"
            onClicked: root.phase = "proposed"
        }
        H.InkButton {
            text: root.phase === "proposed" || root.phase === "editing" ? "no" : "close"
            ink: "c1"
            onClicked: root.phase === "proposed" || root.phase === "editing" ? root.no() : root.dismissed()
        }
    }
}
