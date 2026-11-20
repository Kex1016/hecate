import QtQuick
import QtQuick.Layouts
import Quickshell.Services.UPower
import qs.services
import qs.modules.common.widgets.haiiro as H

// `power`: the battery and the power profile. A machine without a battery
// shows its profile alone.
H.HWindow {
    id: root

    readonly property real pct: Battery.percentage
    readonly property real secs: Battery.isCharging ? Battery.timeToFull : Battery.timeToEmpty
    readonly property string span: {
        if (secs <= 0) return "";
        const m = Math.round(secs / 60);
        return Math.floor(m / 60) + "h " + String(m % 60).padStart(2, "0") + "m";
    }

    widget: true
    label: "power"
    implicitWidth: 360

    RowLayout {
        visible: Battery.available
        spacing: 0
        H.HText { role: "nav"; text: Math.round(root.pct * 100) + "%" }
        H.HText { ink: "c1"; text: root.span.length > 0 ? " · " + root.span : "" }
        H.HText { ink: "c3"; text: Battery.isCharging ? " · charging" : "" }
    }
    H.HText { visible: !Battery.available; text: "on mains power" }
    H.Progress {
        visible: Battery.available
        Layout.fillWidth: true
        value: root.pct
    }
    RowLayout {
        spacing: 6
        H.Chip {
            text: "saver"
            on: PowerProfiles.profile === PowerProfile.PowerSaver
            interactive: true
            onClicked: PowerProfiles.profile = PowerProfile.PowerSaver
        }
        H.Chip {
            text: "balanced"
            on: PowerProfiles.profile === PowerProfile.Balanced
            interactive: true
            onClicked: PowerProfiles.profile = PowerProfile.Balanced
        }
        H.Chip {
            visible: PowerProfiles.hasPerformanceProfile
            text: "perf"
            on: PowerProfiles.profile === PowerProfile.Performance
            interactive: true
            onClicked: PowerProfiles.profile = PowerProfile.Performance
        }
    }
}
