pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// Unread notifications as a love ink block ("3"), shown only when there
// are some; do not disturb adds a muted "dnd" before it. Hover: the latest
// three; click: the quick panel, where they are read.
BarSegment {
    id: root

    readonly property int unread: Notifications.unread
    readonly property var latest: Notifications.list.slice(-3).reverse()

    visible: root.unread > 0 || Notifications.silent
    popupLabel: "notify"
    popupWidth: 360
    popupBlocked: root.unread === 0
    onClicked: GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen

    HText {
        visible: Notifications.silent
        text: "dnd"
        ink: "muted"
    }

    Rectangle {
        visible: root.unread > 0
        width: count.implicitWidth + 2 * HTokens.ch()
        height: count.implicitHeight
        color: HTokens.love

        HText {
            id: count
            x: HTokens.ch()
            text: root.unread
            ink: "surface"
        }
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space2

            Repeater {
                model: root.latest

                Column {
                    id: note
                    required property var modelData
                    Layout.fillWidth: true

                    HText {
                        width: note.width
                        text: BarState.appName(note.modelData.appName) || "notify"
                        ink: "foam"
                        elide: Text.ElideRight
                    }
                    HText {
                        width: note.width
                        text: BarState.oneLine(note.modelData.summary)
                        elide: Text.ElideRight
                    }
                }
            }
            HText {
                Layout.fillWidth: true
                visible: root.unread > root.latest.length
                text: root.latest.length > 0 ? "and " + (root.unread - root.latest.length) + " more" : root.unread + " unread"
                ink: "inkMuted"
                role: "caption"
            }
        }
    }
}
