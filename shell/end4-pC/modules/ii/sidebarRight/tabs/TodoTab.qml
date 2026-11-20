pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell

// What's left to do: type after the prompt and press enter to add one, click
// a task to tick it off, del removes it. Done tasks sink to the bottom.
ColumnLayout {
    id: root

    // keep each task's index in Todo.list; open ones first
    readonly property var tasks: Todo.list.map((t, i) => ({ content: t.content, done: t.done, index: i })).sort((a, b) => a.done - b.done)

    spacing: H.HTokens.space1

    H.PromptLine {
        id: add
        Layout.fillWidth: true
        path: "~/todo"
        input: true
        onAccepted: {
            const t = add.text.trim();
            if (t.length > 0)
                Todo.addTask(t);
            add.text = "";
        }
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: root.tasks
        }
        delegate: Item {
            id: task
            required property var modelData
            width: list.width
            implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody) + 6

            Rectangle {
                anchors.fill: parent
                visible: mouse.containsMouse
                color: H.HTokens.overlay
            }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: H.HTokens.space1
                anchors.rightMargin: H.HTokens.space1
                spacing: H.HTokens.ch()

                H.HText {
                    text: task.modelData.done ? "[x]" : "[ ]"
                    ink: task.modelData.done ? "c1" : "c3"
                }
                H.HText {
                    Layout.fillWidth: true
                    text: task.modelData.content
                    ink: task.modelData.done ? "c1" : "text"
                    font.strikeout: task.modelData.done
                    elide: Text.ElideRight
                }
            }

            MouseArea {
                id: mouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: task.modelData.done ? Todo.markUnfinished(task.modelData.index) : Todo.markDone(task.modelData.index)
            }

            H.InkButton {
                visible: mouse.containsMouse || hovered
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                ink: "c2"
                text: "del"
                onClicked: Todo.deleteItem(task.modelData.index)
            }
        }

        H.HText {
            visible: list.count === 0
            width: list.width
            text: "Nothing to do. You could rest, for once."
            ink: "c4"
            wrapMode: Text.Wrap
        }
    }
}
