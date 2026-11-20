import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H

// `vim: ~/todo.md`: the to-do list, shared with the quick panel. A done
// task is a chip that is on. Click a chip to flip it, right-click to drop
// it; type under the chips and press enter to add one.
H.HWindow {
    id: root

    readonly property var items: Todo.list
    readonly property int doneCount: items.filter(t => t.done).length

    widget: true
    label: "vim: ~/todo.md"
    implicitWidth: 380

    Flow {
        Layout.fillWidth: true
        spacing: 8
        Repeater {
            model: root.items
            delegate: H.Chip {
                id: chip
                required property var modelData
                required property int index
                text: chip.modelData.content
                on: chip.modelData.done
                interactive: true
                onClicked: chip.modelData.done ? Todo.markUnfinished(chip.index) : Todo.markDone(chip.index)
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.RightButton
                    onClicked: Todo.deleteItem(chip.index)
                }
            }
        }
    }
    H.HText {
        visible: root.items.length === 0
        ink: "c1"
        text: "nothing to do"
    }
    Item {
        Layout.fillWidth: true
        implicitHeight: H.HTokens.lineBox(H.HTokens.fontLabel)
        H.HText { id: plus; role: "caption"; ink: "c3"; text: "+" }
        TextInput {
            id: input
            x: plus.implicitWidth + 8
            width: parent.width - x
            height: parent.height
            verticalAlignment: TextInput.AlignVCenter
            color: H.HTokens.text
            font.family: H.HTokens.mono
            font.pixelSize: H.HTokens.fontLabel
            selectionColor: H.HTokens.pine
            selectedTextColor: H.HTokens.text
            clip: true
            onAccepted: {
                const t = text.trim();
                if (t.length > 0) Todo.addTask(t);
                text = "";
            }
            Keys.onEscapePressed: { text = ""; focus = false; }
            onActiveFocusChanged: GlobalStates.desktopWidgetKeyboardFocus = activeFocus
            Component.onDestruction: if (activeFocus) GlobalStates.desktopWidgetKeyboardFocus = false
            Rectangle {
                visible: input.activeFocus
                x: input.cursorRectangle.x
                y: 1
                width: H.HTokens.ch(H.HTokens.fontLabel)
                height: parent.height - 2
                color: H.HTokens.text
                z: -1
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.IBeamCursor
            onPressed: mouse => {
                GlobalStates.desktopWidgetKeyboardFocus = true;
                input.forceActiveFocus();
                mouse.accepted = false;
            }
        }
    }
    H.HText {
        role: "caption"
        ink: "c1"
        text: root.doneCount + " of " + root.items.length + " · enter adds one"
    }
}
