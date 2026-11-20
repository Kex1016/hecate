pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// airpods: <model> · each bud's battery and the case's, the noise mode as chips, and the
// pause-when-one-comes-out rule. From librepods (services/apple/AirPods.qml). The same
// window serves the desktop, the bar segment's hover and the quick panel's chip.
H.HWindow {
    id: root

    property bool live: true

    // "AirPods Pro" -> "pro", "AirPods Max" -> "max", "AirPods (3rd generation)" -> "3"
    readonly property string model: {
        const name = AirPods.name.toLowerCase();
        const m = name.match(/airpods\s*(pro|max|\(?\d)/);
        if (m) return m[1].replace("(", "");
        return name.length > 0 ? name : "airpods";
    }

    function percent(v) {
        return v >= 0 ? `${v}%` : "--";
    }

    label: AirPods.connected ? `airpods: ${root.model}` : "airpods"
    widget: true
    implicitWidth: 360

    // left 84%   right 81%   case 32%, spread over the width like the board
    RowLayout {
        Layout.fillWidth: true
        visible: AirPods.connected && !AirPods.isHeadset
        spacing: 0
        Repeater {
            model: [
                { key: "left", value: AirPods.left, ink: "c6", where: AirPods.leftState },
                { key: "right", value: AirPods.right, ink: "c6", where: AirPods.rightState },
                { key: "case", value: AirPods.caseLevel, ink: "c3", where: "" },
            ]
            delegate: RowLayout {
                id: cell
                required property var modelData
                required property int index
                spacing: H.HTokens.ch(14)
                // a filler before every cell but the first spreads them out
                Item {
                    visible: cell.index > 0
                    Layout.fillWidth: true
                }
                H.HText {
                    role: "code"
                    // a bud that is out of the ear (not in the case) reads muted
                    ink: cell.modelData.where === "out" ? "c1" : "text"
                    text: cell.modelData.key
                }
                H.HText {
                    role: "code"
                    ink: cell.modelData.value >= 0 && cell.modelData.value <= 20 ? "c2" : cell.modelData.ink
                    text: root.percent(cell.modelData.value)
                }
            }
        }
    }

    // AirPods Max: one level
    RowLayout {
        visible: AirPods.connected && AirPods.isHeadset
        spacing: H.HTokens.ch(14)
        H.HText { role: "code"; text: "battery" }
        H.HText { role: "code"; ink: "c6"; text: root.percent(AirPods.headset) }
    }

    Flow {
        Layout.fillWidth: true
        visible: AirPods.connected || AirPods.external
        spacing: 6
        Repeater {
            model: AirPods.modes
            delegate: H.Chip {
                required property var modelData
                text: modelData.label
                on: AirPods.noiseMode === modelData.id
                interactive: true
                onClicked: AirPods.setNoiseMode(modelData.id)
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: AirPods.connected
        role: "caption"
        ink: "c1"
        wrapMode: Text.Wrap
        text: (AirPods.pauseOnRemove ? "take one out to pause" : "taking one out keeps playing") + " · bar shows the lower bud"
    }

    // what is missing, in one line
    H.HText {
        Layout.fillWidth: true
        visible: !AirPods.connected
        role: "code"
        ink: "c1"
        wrapMode: Text.Wrap
        text: !AirPods.installed ? "librepods is not installed · cakeos.hardware airpods"
            : AirPods.external ? "librepods is running on its own, so the battery can't be read"
            : !AirPods.wanted ? "airpods are off in settings"
            : "not connected · open the case near this computer"
    }

    RowLayout {
        visible: AirPods.external || (AirPods.running && AirPods.connected)
        spacing: 8
        H.InkButton {
            visible: AirPods.external
            ink: "c3"
            role: "code"
            text: "show battery"
            onClicked: AirPods.takeOver()
        }
        H.InkButton {
            ink: "c1"
            role: "code"
            text: "librepods"
            onClicked: AirPods.openApp()
        }
    }
}
