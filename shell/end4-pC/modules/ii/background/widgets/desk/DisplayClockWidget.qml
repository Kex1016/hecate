import QtQuick
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H

// The display clock: the time in the display face at 150px with a hard
// 8px base shadow, the date under it. No window.
Item {
    id: root

    property bool live: true
    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }

    readonly property date now: clock.date
    readonly property bool h12: DateTime.use12HourFormat
    readonly property string hh: {
        const h = h12 ? (now.getHours() % 12 || 12) : now.getHours();
        return String(h).padStart(2, "0");
    }
    readonly property string mm: String(now.getMinutes()).padStart(2, "0")
    readonly property string suffix: h12 ? (now.getHours() < 12 ? " am" : " pm") : ""
    readonly property string dateText: Qt.locale("en_US").toString(now, "dddd, d MMMM").toLowerCase()

    implicitWidth: Math.max(face.implicitWidth, dateLine.implicitWidth) + 8
    implicitHeight: face.implicitHeight + 10 + dateLine.implicitHeight + 8

    H.HText {
        x: face.x + 8; y: 8
        role: "display"
        ink: "base"
        text: face.text
    }
    H.HText {
        id: face
        anchors.horizontalCenter: parent.horizontalCenter
        role: "display"
        text: root.hh + ":" + root.mm + root.suffix
    }
    H.HText {
        id: dateLine
        anchors.horizontalCenter: parent.horizontalCenter
        y: face.implicitHeight + 10
        role: "nav"
        text: root.dateText
    }
}
