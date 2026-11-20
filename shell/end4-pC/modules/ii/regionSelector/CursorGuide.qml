import qs.modules.common.widgets.haiiro as H
import QtQuick

// The tag that follows the cursor: which mode a click will use.
Item {
    id: root
    property var action
    property var selectionMode

    readonly property string mode: RegionModes.idOf(root.action)
    readonly property string suffix: root.selectionMode === RegionSelection.SelectionMode.ScreenTarget ? " · screen"
        : root.selectionMode === RegionSelection.SelectionMode.Circle ? " · free" : ""

    property int margins: 8
    implicitWidth: tag.implicitWidth + margins * 2
    implicitHeight: tag.implicitHeight + margins * 2

    Rectangle {
        id: tag
        x: root.margins
        y: root.margins
        implicitWidth: label.implicitWidth + 16
        implicitHeight: label.implicitHeight + 4
        color: H.HTokens.surface

        H.HText {
            id: label
            x: 8
            y: 2
            role: "caption"
            ink: "c5"
            text: root.mode + root.suffix
        }
    }
}
