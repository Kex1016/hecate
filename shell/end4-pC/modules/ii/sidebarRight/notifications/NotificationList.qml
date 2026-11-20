pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell

// NOTIFICATIONS [n] and clear all, then every notification, newest first.
ColumnLayout {
    id: root

    readonly property var items: [...Notifications.list].sort((a, b) => b.time - a.time)

    spacing: 14

    RowLayout {
        Layout.fillWidth: true
        spacing: 0

        H.HLabel {
            text: "notifications"
        }
        H.Chip {
            visible: root.items.length > 0
            Layout.leftMargin: H.HTokens.space2
            badge: true
            text: root.items.length
        }
        Item {
            Layout.fillWidth: true
        }
        H.InkButton {
            visible: root.items.length > 0
            flat: true
            text: "clear all"
            onClicked: Notifications.discardAllNotifications()
        }
    }

    ListView {
        id: list
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        spacing: 10
        boundsBehavior: Flickable.StopAtBounds
        model: ScriptModel {
            values: root.items
        }
        delegate: NotificationCard {
            required property var modelData
            width: list.width
            notif: modelData
        }

        H.HText {
            visible: root.items.length === 0
            width: list.width
            text: Notifications.silent ? "Do not disturb is on. Whatever arrives waits here." : "Nothing new. The computer hums quietly."
            ink: "c4"
            wrapMode: Text.Wrap
        }
    }
}
