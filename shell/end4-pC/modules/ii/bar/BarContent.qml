pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.modules.common.widgets.haiiro

// The bar's three groups in their fixed order. Left: the ☼ mark, the
// workspaces and the focused window. Centre: the clock and the date, always
// in the middle of the screen. Right: media, tray, updates, keyboard
// layout, volume, network, battery, unread. Left and right share what the
// centre leaves; the window title and the track title give way first.
Item {
    id: root

    required property ShellScreen screen

    readonly property int pad: HTokens.space4
    readonly property int groupGap: HTokens.space6
    readonly property int segmentGap: HTokens.space4
    readonly property int sideWidth: Math.max(0, Math.floor((area.width - centre.width) / 2) - root.groupGap)

    // the 3px seam under the bar
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: HTokens.barSeam
        color: HTokens.overlay
    }

    Item {
        id: area
        x: root.pad
        width: parent.width - 2 * root.pad
        height: parent.height - HTokens.barSeam

        RowLayout {
            id: left
            x: 0
            width: root.sideWidth
            height: parent.height
            spacing: root.segmentGap

            Logo {}
            Workspaces {
                screen: root.screen
            }
            ActiveWindow {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
            }
        }

        ClockWidget {
            id: centre
            anchors.horizontalCenter: parent.horizontalCenter
            height: parent.height
        }

        RowLayout {
            id: right
            x: parent.width - width
            width: root.sideWidth
            height: parent.height
            spacing: root.segmentGap

            Item {
                Layout.fillWidth: true
            }
            Media {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.maximumWidth: implicitWidth
            }
            SysTray {}
            UpdatesCount {}
            KeyboardLayout {}
            Volume {}
            NetworkIndicator {}
            BatteryIndicator {}
            NotificationUnreadCount {}
        }
    }
}
