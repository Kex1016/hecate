pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A choice of two to four words, the current one on a pine band:
//   Segmented { model: ["blobs", "wallpaper"]; currentIndex: 0; onActivated: i => ... }
// Left/Right move the choice. Numbers use Spin; on/off settings use a Chip.
Item {
    id: root

    property var model: []
    property int currentIndex: 0
    signal activated(int index)

    activeFocusOnTab: enabled
    opacity: enabled ? 1 : 0.55
    implicitWidth: segs.implicitWidth
    implicitHeight: segs.implicitHeight

    function choose(i) {
        if (i < 0 || i >= root.model.length || i === root.currentIndex) return;
        root.currentIndex = i;
        root.activated(i);
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Left) root.choose(root.currentIndex - 1);
        else if (event.key === Qt.Key_Right) root.choose(root.currentIndex + 1);
        else return;
        event.accepted = true;
    }

    Rectangle {
        visible: root.activeFocus
        anchors.fill: parent
        anchors.margins: -4
        color: "transparent"
        border.width: 2
        border.color: HTokens.foam
    }

    Row {
        id: segs

        Repeater {
            model: root.model
            Rectangle {
                id: seg
                required property int index
                required property string modelData
                readonly property bool on: index === root.currentIndex
                width: word.implicitWidth + 2 * HTokens.ch()
                height: word.implicitHeight
                color: on ? HTokens.pine : "transparent"

                HText {
                    id: word
                    anchors.centerIn: parent
                    text: seg.modelData
                    ink: seg.on || mouse.containsMouse ? "text" : "subtle"
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.choose(seg.index)
                }
            }
        }
    }
}
