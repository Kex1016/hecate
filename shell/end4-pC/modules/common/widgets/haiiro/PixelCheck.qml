import QtQuick
import qs.modules.common.widgets.haiiro

// A checkbox drawn as a square, for the vim todo: 12px with a 1px text
// outline; checked, it is pine with a 2px base ring inside. The square is
// the picture, the whole item is the target: give it the row's height.
//   PixelCheck { checked: item.done; onToggled: Todo.toggle(index) }
PixelGrid {
    id: root

    property bool checked: false
    signal toggled()

    readonly property real side: px(12)
    readonly property real line: px(1)
    readonly property real ring: px(2)

    implicitWidth: side
    implicitHeight: side
    activeFocusOnTab: true

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.toggled();
            event.accepted = true;
        }
    }

    Item {
        y: Math.round((root.height - root.side) / 2 * root.dpr) / root.dpr
        width: root.side
        height: root.side

        // the outline, as four edges: a border cannot be a fraction of a pixel wide
        Rectangle { width: parent.width; height: root.line; color: HTokens.text; visible: !root.checked }
        Rectangle { y: parent.height - height; width: parent.width; height: root.line; color: HTokens.text; visible: !root.checked }
        Rectangle { width: root.line; height: parent.height; color: HTokens.text; visible: !root.checked }
        Rectangle { x: parent.width - width; width: root.line; height: parent.height; color: HTokens.text; visible: !root.checked }
        Rectangle { anchors.fill: parent; color: HTokens.pine; visible: root.checked }
        Rectangle {
            visible: root.checked
            anchors.fill: parent
            anchors.margins: root.line
            color: HTokens.base
        }
        Rectangle {
            visible: root.checked
            anchors.fill: parent
            anchors.margins: root.line + root.ring
            color: HTokens.pine
        }
        Rectangle {
            visible: root.activeFocus
            anchors.fill: parent
            anchors.margins: -4
            color: "transparent"
            border.width: 2
            border.color: HTokens.foam
        }
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
