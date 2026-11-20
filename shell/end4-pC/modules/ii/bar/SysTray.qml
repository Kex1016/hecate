pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.SystemTray
import qs.services

// The tray: 16px icons 8px apart, dithered to two inks because none of
// them has a haiiro version. Hover names the item; left click activates
// it, right click opens its menu as a window. Which items show is
// TrayService's call (the tray.* keys).
Row {
    id: root

    // The lock screen shows only the items it picks; the bar shows them all.
    property list<var> pinnedItems: TrayService.pinnedItems
    // kept for the lock screen, which sets them; the bar has no overflow
    property bool showSeparator: false
    property bool showOverflowMenu: false

    visible: root.pinnedItems.length > 0
    spacing: 8
    Layout.alignment: Qt.AlignVCenter

    Repeater {
        model: ScriptModel {
            values: root.pinnedItems
        }

        SysTrayItem {
            required property SystemTrayItem modelData
            item: modelData
        }
    }
}
