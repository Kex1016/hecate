import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "SkyMath.js" as SkyMath

// `date`: the compact clock. Seconds in a muted ink, the weekday and day,
// the week and the day of the year.
H.HWindow {
    id: root

    property bool live: true
    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Seconds
    }
    readonly property date now: clock.date
    readonly property bool h12: DateTime.use12HourFormat
    readonly property string hhmm: {
        const h = h12 ? (now.getHours() % 12 || 12) : now.getHours();
        return String(h).padStart(2, "0") + ":" + String(now.getMinutes()).padStart(2, "0");
    }
    readonly property var en: Qt.locale("en_US")

    widget: true
    label: "date"
    implicitWidth: 360

    RowLayout {
        spacing: 0
        H.HText { role: "nav"; text: root.hhmm }
        H.HText { role: "nav"; ink: "c1"; text: ":" + String(root.now.getSeconds()).padStart(2, "0") }
        H.HText { role: "caption"; ink: "c1"; visible: root.h12; Layout.leftMargin: 8; text: root.now.getHours() < 12 ? "am" : "pm" }
    }
    RowLayout {
        spacing: 0
        H.HText { text: root.en.toString(root.now, "dddd").toLowerCase() + " " }
        H.HText { ink: "c6"; text: String(root.now.getDate()) }
        H.HText { text: " " + root.en.toString(root.now, "MMMM").toLowerCase() }
    }
    H.HText {
        role: "caption"
        ink: "c1"
        text: "week " + SkyMath.isoWeek(root.now) + " · day " + SkyMath.dayOfYear(root.now) + " of " + SkyMath.daysInYear(root.now)
    }
}
