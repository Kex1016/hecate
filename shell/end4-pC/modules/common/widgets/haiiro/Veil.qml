import QtQuick
import qs.modules.common.widgets.haiiro

// Dims the desktop behind a panel: a 2x2 checker of base over whatever is
// behind, never blur, never a fade. `heavy` adds veil-heavy under the
// checker, for full-screen panels (dashboard, session menu). It takes the
// clicks that land on it, so a click outside the panel can close it.
Item {
    id: root

    property bool heavy: false
    property bool blocking: true
    signal clicked()

    Rectangle {
        anchors.fill: parent
        visible: root.heavy
        color: HTokens.veilHeavy
    }

    Dither {
        anchors.fill: parent
        ink: HTokens.base
        ground: "transparent"
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.blocking
        acceptedButtons: Qt.AllButtons
        onClicked: root.clicked()
        onWheel: wheel => wheel.accepted = true
    }
}
