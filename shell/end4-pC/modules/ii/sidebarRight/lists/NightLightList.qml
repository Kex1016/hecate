import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// Night light and the screen's dimming: on or off, on its own schedule,
// how warm, and the gamma that dims below the backlight.
ColumnLayout {
    id: root

    readonly property var actions: []

    spacing: 0

    H.SettingRow {
        Layout.fillWidth: true
        name: "night light"
        description: Hyprsunset.temperatureActive ? "Warm screen, " + Hyprsunset.colorTemperature + "K." : "Off."
        H.Chip {
            interactive: true
            on: Hyprsunset.temperatureActive
            text: Hyprsunset.temperatureActive ? "on" : "off"
            onClicked: Hyprsunset.toggleTemperature()
        }
    }

    H.SettingRow {
        Layout.fillWidth: true
        name: "automatic"
        description: "From " + Config.options.light.night.from + " to " + Config.options.light.night.to + "."
        H.Chip {
            interactive: true
            on: Config.options.light.night.automatic
            text: Config.options.light.night.automatic ? "on" : "off"
            onClicked: Config.options.light.night.automatic = !Config.options.light.night.automatic
        }
    }

    QuickSliderRow {
        label: "warmth"
        // lower Kelvin is warmer: the track runs from cool to warm
        from: 6500
        to: 1200
        stepSize: -100
        value: Config.options.light.night.colorTemperature
        valueText: Math.round(value / 100) * 100 + "K"
        onMoved: v => Config.options.light.night.colorTemperature = Math.round(v)
    }

    QuickSliderRow {
        label: "dim"
        from: Hyprsunset.gammaLowerLimit
        to: 100
        stepSize: 5
        value: Hyprsunset.gamma
        valueText: Math.round(value) + "%"
        onMoved: v => Hyprsunset.setGamma(Math.round(v))
    }

    Item {
        Layout.fillHeight: true
    }

    component QuickSliderRow: ColumnLayout {
        id: row
        property alias label: name.text
        property alias from: slider.from
        property alias to: slider.to
        property alias stepSize: slider.stepSize
        property alias value: slider.value
        property alias valueText: slider.valueText
        signal moved(real v)
        Layout.fillWidth: true
        spacing: H.HTokens.space1

        H.HRule {
            Layout.fillWidth: true
            Layout.topMargin: 2
        }
        RowLayout {
            Layout.fillWidth: true
            H.HText {
                id: name
                Layout.fillWidth: true
            }
            H.HText {
                text: slider.valueText
                role: "code"
                ink: "inkMuted"
            }
        }
        H.Slider {
            id: slider
            Layout.fillWidth: true
            Layout.bottomMargin: H.HTokens.space2
            showValue: false
            onMoved: v => row.moved(v)
        }
    }
}
