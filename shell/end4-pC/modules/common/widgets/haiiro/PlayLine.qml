pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A dotted playhead line: what has played as a 2px text line, a rose 8px
// square where the song is, the rest dotted in subtle. `seek(position)`
// fires on a click, with the place clicked as 0-1.
//   PlayLine { width: 300; progress: 0.46 }
PixelGrid {
    id: root

    // 0-1
    property real progress: 0
    property bool seekable: false
    signal seek(real position)

    readonly property real head: px(HTokens.pixelCell)
    readonly property real line: px(2)
    readonly property real dot: px(2)
    readonly property real doneW: Math.round(Math.max(0, Math.min(1, progress)) * Math.max(0, width - head) * dpr) / dpr

    implicitWidth: 200
    implicitHeight: head

    Rectangle {
        y: (root.head - root.line) / 2
        width: root.doneW
        height: root.line
        color: HTokens.text
    }
    Rectangle {
        x: root.doneW
        width: root.head
        height: root.head
        color: HTokens.rose
    }
    Repeater {
        model: Math.max(0, Math.ceil((root.width - root.doneW - root.head) / (2 * root.dot)))
        Rectangle {
            required property int index
            x: root.doneW + root.head + index * 2 * root.dot
            y: (root.head - root.line) / 2
            width: Math.min(root.dot, Math.max(0, root.width - x))
            height: root.line
            color: HTokens.subtle
        }
    }
    MouseArea {
        anchors.fill: parent
        anchors.topMargin: -8
        anchors.bottomMargin: -8
        enabled: root.seekable
        cursorShape: root.seekable ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: mouse => root.seek(Math.max(0, Math.min(1, mouse.x / root.width)))
    }
}
