pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.UPower
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// "bat 87%", the value in foam, love when low and not charging, with a +
// while charging. Only on machines with a battery. Hover: the charge, the
// time left and the power profile chips; click: the quick panel.
BarSegment {
    id: root

    readonly property int percent: Math.round(Battery.percentage * 100)
    readonly property bool low: Battery.isLowAndNotCharging
    readonly property string charge: Battery.isCharging ? "charging"
        : Battery.chargeState === UPowerDeviceState.FullyCharged ? "full"
        : Battery.isPluggedIn ? "plugged in" : "discharging"

    visible: Battery.available
    popupLabel: "bat"
    popupWidth: 300
    popupVariant: root.low ? "love" : "pine"
    onClicked: GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen

    HText {
        text: "bat"
    }
    HText {
        text: root.percent + "%" + (Battery.isCharging ? "+" : "")
        ink: root.low ? "love" : "foam"
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space2

            Progress {
                Layout.fillWidth: true
                label: root.charge
                count: root.percent + "%"
                value: Battery.percentage
            }
            Column {
                id: left
                readonly property real seconds: Battery.isCharging ? Battery.timeToFull : Battery.timeToEmpty
                Layout.fillWidth: true
                visible: left.seconds > 0

                HLabel {
                    text: Battery.isCharging ? "until full" : "time left"
                }
                HText {
                    text: BarState.duration(left.seconds)
                    role: "valueLg"
                }
            }
            Row {
                spacing: HTokens.space2

                Repeater {
                    model: [
                        { name: "saver", profile: PowerProfile.PowerSaver },
                        { name: "balanced", profile: PowerProfile.Balanced },
                        { name: "perf", profile: PowerProfile.Performance }
                    ].filter(p => p.profile !== PowerProfile.Performance || PowerProfiles.hasPerformanceProfile)

                    Chip {
                        required property var modelData
                        text: modelData.name
                        on: PowerProfiles.profile === modelData.profile
                        interactive: true
                        onClicked: PowerProfiles.profile = modelData.profile
                    }
                }
            }
        }
    }
}
