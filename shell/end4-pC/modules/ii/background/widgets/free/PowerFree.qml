import QtQuick
import QtQuick.Layouts
import Quickshell.Services.UPower
import qs.services
import qs.modules.common.widgets.haiiro as H

// `power`: a pixel battery and the percentage in Jacquard. The power profile
// is three words in ink, the current one in pine. A machine without a
// battery shows its profile alone.
H.FreeWidget {
    id: root

    readonly property real pct: Battery.percentage
    readonly property real secs: Battery.isCharging ? Battery.timeToFull : Battery.timeToEmpty
    readonly property string span: {
        if (secs <= 0) return "";
        const m = Math.round(secs / 60);
        return Math.floor(m / 60) + "h " + String(m % 60).padStart(2, "0") + "m";
    }

    label: "power"
    spacing: 8

    RowLayout {
        visible: Battery.available
        spacing: 16
        H.PixelBattery {
            Layout.alignment: Qt.AlignVCenter
            level: root.pct
            ink: Battery.isLow ? "love" : "foam"
        }
        H.FreeText {
            Layout.alignment: Qt.AlignVCenter
            role: "display"
            size: 72
            text: Math.round(root.pct * 100) + "%"
        }
    }
    H.FreeText {
        visible: !Battery.available
        role: "nav"
        text: "on mains power"
    }
    H.FreeText {
        visible: Battery.available && (root.span.length > 0 || Battery.isCharging)
        role: "code"
        ink: "subtle"
        text: (Battery.isCharging ? "charging · " : "") + root.span + (root.span.length > 0 ? (Battery.isCharging ? " to full" : " left") : "")
    }
    RowLayout {
        spacing: 4
        Repeater {
            model: [
                { name: "saver", profile: PowerProfile.PowerSaver },
                { name: "balanced", profile: PowerProfile.Balanced },
                { name: "perf", profile: PowerProfile.Performance }
            ]
            delegate: H.InkButton {
                required property var modelData
                readonly property bool current: PowerProfiles.profile === modelData.profile
                visible: modelData.profile !== PowerProfile.Performance || PowerProfiles.hasPerformanceProfile
                text: modelData.name
                flat: !current
                ink: current ? "c5" : ""
                onClicked: PowerProfiles.profile = modelData.profile
            }
        }
    }
}
