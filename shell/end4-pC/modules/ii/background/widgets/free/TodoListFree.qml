pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common.widgets.haiiro as H

// `vim ~/todo.md`: the to-do list, shared with the quick panel, as a vim
// buffer: line numbers, a square checkbox, done lines struck through, a
// tilde for the rest. Click a line to flip it, right-click to drop it;
// click the tilde and type to add one (enter adds it).
H.FreeWidget {
    id: root

    readonly property var items: Todo.list
    readonly property int doneCount: items.filter(t => t.done).length

    label: "vim ~/todo.md"
    spacing: 8

    ColumnLayout {
        Layout.minimumWidth: 300
        spacing: 2

        Repeater {
            model: root.items
            delegate: Item {
                id: line
                required property var modelData
                required property int index
                Layout.fillWidth: true
                implicitWidth: row.implicitWidth
                implicitHeight: 24

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: mouse => {
                        if (mouse.button === Qt.RightButton) Todo.deleteItem(line.index);
                        else line.modelData.done ? Todo.markUnfinished(line.index) : Todo.markDone(line.index);
                    }
                }
                Row {
                    id: row
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 10
                    H.FreeText {
                        width: 2 * H.HTokens.ch(H.HTokens.fontLabel)
                        height: 24
                        horizontalAlignment: Text.AlignRight
                        role: "caption"
                        ink: "highlightHigh"
                        text: line.index + 1
                    }
                    H.PixelCheck {
                        height: 24
                        checked: line.modelData.done
                        onToggled: line.modelData.done ? Todo.markUnfinished(line.index) : Todo.markDone(line.index)
                    }
                    H.FreeText {
                        height: 24
                        ink: line.modelData.done ? "subtle" : "text"
                        font.strikeout: line.modelData.done
                        text: line.modelData.content
                    }
                }
            }
        }

        // the tilde is where the next task is typed
        Item {
            Layout.fillWidth: true
            implicitHeight: 24

            H.FreeText {
                id: tilde
                x: 2 * H.HTokens.ch(H.HTokens.fontBody)
                height: parent.height
                ink: "pine"
                text: "~"
            }
            // what the tilde is for, until it is clicked
            H.FreeText {
                visible: !input.activeFocus && input.text.length === 0
                x: tilde.x + 2 * H.HTokens.ch(H.HTokens.fontBody)
                height: parent.height
                ink: "muted"
                text: root.items.length === 0 ? "nothing to do · + add one" : "+"
            }
            TextInput {
                id: input
                x: tilde.x + 2 * H.HTokens.ch(H.HTokens.fontBody)
                width: parent.width - x
                height: parent.height
                verticalAlignment: TextInput.AlignVCenter
                color: H.HTokens.text
                font.family: H.HTokens.mono
                font.pixelSize: H.HTokens.fontBody
                font.kerning: false
                selectionColor: H.HTokens.pine
                selectedTextColor: H.HTokens.text
                renderType: Text.NativeRendering
                clip: true
                cursorDelegate: H.BlockCursor {
                    size: H.HTokens.fontBody
                    visible: input.activeFocus
                }
                onAccepted: {
                    const t = text.trim();
                    if (t.length > 0) Todo.addTask(t);
                    text = "";
                }
                Keys.onEscapePressed: { text = ""; focus = false; }
                onActiveFocusChanged: GlobalStates.desktopWidgetKeyboardFocus = activeFocus
                Component.onDestruction: if (activeFocus) GlobalStates.desktopWidgetKeyboardFocus = false
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
    }
    H.FreeText {
        role: "caption"
        ink: "subtle"
        text: root.doneCount + " of " + root.items.length + " · enter adds one"
    }
}
