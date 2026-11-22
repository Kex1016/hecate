import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H

// `fortune`: one line about now, picked from what is true: how long the
// machine has been up, the weather, the battery, the calendar, the hour.
// The line only changes when the hour does, so it never flickers. The one
// widget allowed to be large rose type, with a cursor at the end of it.
H.FreeWidget {
    id: root

    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }

    // "2d, 5h, 3m" from DateTime.uptime
    readonly property real uptimeHours: {
        const u = DateTime.uptime;
        const d = Number((u.match(/(\d+)d/) ?? [])[1] ?? 0);
        const h = Number((u.match(/(\d+)h/) ?? [])[1] ?? 0);
        return d * 24 + h;
    }
    readonly property string weather: String(Weather.data?.description ?? "").toLowerCase()

    readonly property string line: {
        const now = clock.date;
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

    label: "fortune"

    Item {
        id: body
        Layout.preferredWidth: Math.min(520, quote.implicitWidth)
        implicitHeight: quote.implicitHeight

        H.FreeText {
            id: quote
            width: parent.width
            role: "nav"
            ink: "rose"
            wrapMode: Text.WordWrap
            text: root.line
        }
        // the same text, unseen, for where its last character ends
        TextEdit {
            id: probe
            width: quote.width
            height: quote.height
            opacity: 0
            enabled: false
            readOnly: true
            wrapMode: TextEdit.WordWrap
            text: quote.text
            font: quote.font
        }
        // 8x16 after the last character, 8px away, sitting on the baseline
        H.BlockCursor {
            readonly property rect end: probe.positionToRectangle(probe.length)
            ink: "rose"
            x: end.x + 8
            y: end.y + quote.lineHeight - implicitHeight - 7
        }
    }
}
