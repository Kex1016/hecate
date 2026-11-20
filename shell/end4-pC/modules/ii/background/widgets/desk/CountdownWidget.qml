import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H

// `next`: how long until the next calendar event. A gold window, because it
// is the one thing here that is about you being somewhere.
H.HWindow {
    id: root

    property bool live: true
    SystemClock {
        id: clock
        enabled: root.live
        precision: SystemClock.Minutes
    }
    readonly property var upcoming: DesktopWidgets.nextEvent
    readonly property real remaining: upcoming ? upcoming.start - clock.date.getTime() : 0
    readonly property bool has: upcoming !== null && remaining > -60000
    readonly property var en: Qt.locale("en_US")

    function span(ms) {
        const m = Math.max(0, Math.floor(ms / 60000));
        const d = Math.floor(m / 1440), h = Math.floor(m % 1440 / 60), mi = m % 60;
        if (d > 0) return d + "d " + h + "h";
        if (h > 0) return h + "h " + mi + "m";
        return mi + "m";
    }

    widget: true
    variant: "gold"
    label: "next"
    implicitWidth: 360

    H.HText {
        ink: "c3"
        Layout.fillWidth: true
        elide: Text.ElideRight
        text: root.has ? root.upcoming.title : "nothing planned"
    }
    H.HText {
        role: "nav"
        text: root.has ? root.span(root.remaining) : "—"
    }
    H.HText {
        role: "caption"
        ink: "c1"
        text: {
            if (!root.has) return "no event in your calendar";
            const d = new Date(root.upcoming.start);
            return root.en.toString(d, "ddd d MMM").toLowerCase() + " · " + String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0") + " · from " + root.upcoming.source;
        }
    }
}
