import QtQuick
import qs.modules.common.widgets.haiiro

// A number setting: `‹ 5s ›`, stepped with the arrow keys or a click on
// an arrow.
//   Spin { value: 5; from: 1; to: 30; suffix: "s" }
Item {
    id: root

    property real value: 0
    property real from: 0
    property real to: 100
    property real stepSize: 1
    property string suffix: ""
    // override for anything that is not a plain number
    property string text: value + suffix
    signal moved(real value)

    activeFocusOnTab: enabled
    opacity: enabled ? 1 : 0.55
    implicitWidth: spins.implicitWidth
    implicitHeight: spins.implicitHeight

    function step(n) {
        const v = Math.max(root.from, Math.min(root.to, root.value + n * root.stepSize));
        if (v !== root.value) {
            root.value = v;
            root.moved(v);
        }
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Down) root.step(-1);
        else if (event.key === Qt.Key_Right || event.key === Qt.Key_Up) root.step(1);
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
        id: spins
        spacing: HTokens.ch()

        HText {
            text: "‹"
            ink: down.containsMouse ? "text" : "subtle"
            MouseArea {
                id: down
                anchors.fill: parent
                anchors.margins: -4
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.step(-1)
            }
        }
        HText {
            text: root.text
        }
        HText {
            text: "›"
            ink: up.containsMouse ? "text" : "subtle"
            MouseArea {
                id: up
                anchors.fill: parent
                anchors.margins: -4
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.step(1)
            }
        }
    }
}
