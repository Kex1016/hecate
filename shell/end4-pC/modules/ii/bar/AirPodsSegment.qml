pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro
import qs.services.apple

// "pods 81", the lower bud in foam (love at 20% or less), while AirPods are
// connected through librepods. Hover: both buds, the case and the noise
// modes; click: the next noise mode.
BarSegment {
    id: root

    readonly property int level: AirPods.lowerBud

    visible: AirPods.connected
    popupLabel: AirPods.name ? "airpods: " + AirPods.name.toLowerCase() : "airpods"
    popupWidth: 340
    onClicked: {
        const i = AirPods.modes.findIndex(m => m.id === AirPods.noiseMode);
        AirPods.setNoiseMode(AirPods.modes[(i + 1) % AirPods.modes.length].id);
    }

    function percent(v) {
        return v >= 0 ? v + "%" : "—";
    }

    HText {
        text: "pods"
    }
    HText {
        text: root.level >= 0 ? root.level : "—"
        ink: root.level >= 0 && root.level <= 20 ? "love" : "foam"
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space2

            RowLayout {
                Layout.fillWidth: true
                visible: !AirPods.isHeadset
                spacing: HTokens.ch()
                HText { text: "left"; ink: AirPods.leftState === "out" ? "muted" : "text" }
                HText { text: root.percent(AirPods.left); ink: "foam" }
                Item { Layout.fillWidth: true }
                HText { text: "right"; ink: AirPods.rightState === "out" ? "muted" : "text" }
                HText { text: root.percent(AirPods.right); ink: "foam" }
                Item { Layout.fillWidth: true }
                HText { text: "case" }
                HText { text: root.percent(AirPods.caseLevel); ink: "gold" }
            }
            RowLayout {
                visible: AirPods.isHeadset
                spacing: HTokens.ch()
                HText { text: "battery" }
                HText { text: root.percent(AirPods.headset); ink: "foam" }
            }
            Flow {
                Layout.fillWidth: true
                spacing: HTokens.space2
                Repeater {
                    model: AirPods.modes
                    Chip {
                        required property var modelData
                        text: modelData.label
                        on: AirPods.noiseMode === modelData.id
                        interactive: true
                        onClicked: AirPods.setNoiseMode(modelData.id)
                    }
                }
            }
        }
    }
}
