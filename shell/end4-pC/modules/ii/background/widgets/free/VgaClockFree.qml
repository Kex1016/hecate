import QtQuick
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import "../desk/SkyMath.js" as SkyMath

// `date`: the compact clock, the date as lines and the day as a ruler of 24
// hours: past in text, now in pine.
H.FreeWidget {
    id: root

    label: "date"

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

    Row {
        H.FreeText { role: "nav"; text: root.hhmm }
        H.FreeText { role: "nav"; ink: "subtle"; text: ":" + String(root.now.getSeconds()).padStart(2, "0") }
        H.FreeText { role: "caption"; ink: "subtle"; visible: root.h12; leftPadding: 8; anchors.baseline: undefined; text: root.now.getHours() < 12 ? "am" : "pm" }
    }
    Row {
        H.FreeText { text: root.en.toString(root.now, "dddd").toLowerCase() + " "; role: "body" }
        H.FreeText { ink: "foam"; role: "body"; text: String(root.now.getDate()) }
        H.FreeText { role: "body"; text: " " + root.en.toString(root.now, "MMMM").toLowerCase() }
    }
    H.DayRuler { hour: root.now.getHours() }
    H.FreeText {
        role: "code"
        ink: "subtle"
        text: "week " + SkyMath.isoWeek(root.now) + " · day " + SkyMath.dayOfYear(root.now) + " of " + SkyMath.daysInYear(root.now)
    }
}
