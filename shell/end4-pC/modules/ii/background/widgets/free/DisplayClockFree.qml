import QtQuick
import Quickshell
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

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

    // "2d, 5h, 3m" from DateTime.uptime
    readonly property real uptimeHours: {
        const u = DateTime.uptime;
        const d = Number((u.match(/(\d+)d/) ?? [])[1] ?? 0);
        const h = Number((u.match(/(\d+)h/) ?? [])[1] ?? 0);
        return d * 24 + h;
    }
    readonly property string weather: String(Weather.data?.description ?? "").toLowerCase()

    // one line about now; it only changes when the hour does
    readonly property string line: {
        const hour = now.getHours();
        const seed = now.getFullYear() * 1000 + Math.floor(now.getTime() / 3600000);
        const options = [];
        if (uptimeHours >= 3)
            options.push("You have been at this for " + (uptimeHours >= 24 ? Math.floor(uptimeHours / 24) + " days" : uptimeHours + " hours") + ". The tea has gone cold.");
        if (Battery.available && Battery.isLowAndNotCharging)
            options.push("The battery is thin. The charger is somewhere nearby.");
        else if (Battery.available && Battery.isFullAndCharging)
            options.push("The battery is full. It may be unplugged now.");
        if (weather.includes("rain") || weather.includes("drizzle"))
            options.push("It is raining out there. A good reason to stay.");
        else if (weather.includes("snow"))
            options.push("It is snowing. The world has gone quiet.");
        const ev = DesktopWidgets.nextEvent;
        if (ev && ev.start - now.getTime() > 0 && ev.start - now.getTime() < 2 * 3600000)
            options.push(ev.title + " is close. It will not wait for you.");
        if (hour >= 1 && hour < 5)
            options.push("It is very late. The code will still be there tomorrow.");
        else if (hour >= 5 && hour < 9)
            options.push("The day is new. Nothing has gone wrong yet.");
        else if (hour >= 22)
            options.push("The evening is nearly spent.");
        if (options.length === 0)
            options.push("Nothing to report. That is also news.");
        return options[seed % options.length];
    }

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
