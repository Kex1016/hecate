pragma ComponentBehavior: Bound
import qs.services.claude
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// `claude: ~/ask`: the context chips (what will be sent, each one
// switchable), the question, then the answer with its buttons. The footer
// says what the question takes with it, and after sending exactly what went.
H.HWindow {
    id: root

    property int maxHeight: 760

    label: "claude: ~/ask"
    variant: !ClaudeAsk.available && ClaudeAsk.checked ? "gold" : ClaudeAsk.phase === "error" ? "love" : "pine"
    // the note never gets cut: a long one pushes out the tab hint
    hints: [{ key: "enter", text: "ask" }]
        .concat(note.length > 36 ? [] : [{ key: "tab", text: "toggle context" }])
        .concat([ClaudeAsk.command ? { key: "ctrl enter", text: "run" } : { key: "esc", text: "close" }])
    note: !ClaudeAsk.available ? "nothing sent"
        : ClaudeAsk.sentSummary.length > 0 ? `sent: ${ClaudeAsk.sentSummary}`
        : `sends: ${ClaudeAsk.pendingSummary}`

    Connections {
        target: ClaudeAsk
        function onFocusPrompt() {
            prompt.text = "";
            prompt.replaceOnType = false;
            prompt.focusField();
        }
        // a question asked from elsewhere (the launcher) shows on the line
        function onQuestionChanged() {
            if (ClaudeAsk.question.length > 0 && prompt.text !== ClaudeAsk.question)
                prompt.text = ClaudeAsk.question;
        }
        function onPhaseChanged() {
            // a finished answer: typing starts a follow-up in place of the question
            if (ClaudeAsk.phase === "done" || ClaudeAsk.phase === "error") {
                prompt.focusField();
                prompt.replaceOnType = true;
            }
        }
    }

    function send() {
        if (ClaudeAsk.busy)
            return;
        ClaudeAsk.ask(prompt.text);
    }

    // the keyboard's gold button
    function run() {
        if (ClaudeAsk.phase === "done" && ClaudeAsk.command)
            ClaudeAsk.runCommand();
    }

    // tab walks the question → chips → the question; shift tab the other way
    function chipItems() {
        const out = [];
        for (let i = 0; i < chipRepeater.count; i++)
            out.push(chipRepeater.itemAt(i));
        return out;
    }
    // `from`: the chip that has focus, -1 for the question
    function focusChip(delta, from) {
        const items = root.chipItems();
        if (items.length === 0)
            return;
        const i = from < 0 ? (delta > 0 ? 0 : items.length - 1) : from + delta;
        if (i < 0 || i >= items.length)
            prompt.focusField();
        else
            items[i].forceActiveFocus();
    }

    Keys.onPressed: event => {
        const ctrl = (event.modifiers & Qt.ControlModifier) !== 0;
        if (event.key === Qt.Key_Escape) {
            ClaudeAsk.close();
            event.accepted = true;
        } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
            root.focusChip(event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier) ? -1 : 1, -1);
            event.accepted = true;
        } else if (ctrl && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.run();
            event.accepted = true;
        } else if (ctrl && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
            ClaudeAsk.selectCommand(event.key === Qt.Key_Up ? -1 : 1);
            event.accepted = true;
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 10

        // what would be sent
        Flow {
            Layout.fillWidth: true
            spacing: 6

            H.HLabel {
                text: "sending"
                height: chipRepeater.count > 0 ? (chipRepeater.itemAt(0)?.height ?? implicitHeight) : implicitHeight
                rightPadding: 4
            }
            Repeater {
                id: chipRepeater
                // by count, so a toggle updates the chips in place and keeps focus
                model: ClaudeAsk.chips.length
                H.Chip {
                    required property int index
                    readonly property var modelData: ClaudeAsk.chips[index] ?? ({})
                    text: modelData.label
                    on: modelData.on
                    interactive: true
                    enabled: ClaudeAsk.available && !ClaudeAsk.busy
                    onClicked: ClaudeAsk.toggleChip(modelData.id)
                    // before Qt's own tab chain, which would skip the question
                    Keys.onTabPressed: event => { root.focusChip(1, index); event.accepted = true; }
                    Keys.onBacktabPressed: event => { root.focusChip(-1, index); event.accepted = true; }
                }
            }
            H.HText {
                visible: chipRepeater.count === 0
                text: "nothing but the question"
                role: "caption"
                ink: "inkMuted"
            }
        }

        AskPrompt {
            id: prompt
            Layout.fillWidth: true
            onAccepted: root.send()
            onRunRequested: root.run()
        }

        H.HRule {
            Layout.fillWidth: true
            visible: answer.visible
        }

        AskAnswer {
            id: answer
            Layout.fillWidth: true
            maxHeight: root.maxHeight - 160
            visible: ClaudeAsk.phase !== "compose" || (!ClaudeAsk.available && ClaudeAsk.checked)
        }
    }
}
