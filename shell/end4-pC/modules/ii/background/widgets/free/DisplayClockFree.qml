import QtQuick
import Quickshell
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import "Narration.js" as Narration

// The display clock out of its box: the time in the display face at 150px
// with the hard 8px shadow, the date at 32px, the day as a ruler of 24
// hours, and, when background.clock.narration is on, the line of narration
// in rose ending in a block cursor. No label: it is dragged by all of it.
H.FreeWidget {
    id: root

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
    readonly property bool narrate: Config.options.background.clock.narration

    // one line about now; it only changes when the hour does
    readonly property string line: Narration.line(root.now, {
        uptime: DateTime.uptime,
        weather: Weather.data?.description,
        battery: Battery,
        nextEvent: DesktopWidgets.nextEvent
    })

    spacing: 10

    H.FreeText {
        role: "display"
        text: root.hh + ":" + root.mm + root.suffix
    }
    H.FreeText {
        role: "nav"
        text: root.dateText
    }
    H.DayRuler {
        hour: root.now.getHours()
        labels: false
    }
    Row {
        visible: root.narrate
        spacing: 8
        H.FreeText {
            anchors.verticalCenter: parent.verticalCenter
            role: "code"
            ink: "rose"
            text: root.line
        }
        H.BlockCursor {
            anchors.verticalCenter: parent.verticalCenter
            ink: "rose"
            size: 16
        }
    }
}
