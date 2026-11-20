import QtQuick
import qs.modules.ii.settings

SettingsPage {
    note: "saved to ~/.config/hecate/shell.toml"

    SetColumn {
        SetSection { text: "pointer" }
        SetChoice {
            name: "acceleration"
            hecateKey: "input.type:pointer.accel_profile"
            fallback: "adaptive"
            values: ["flat", "adaptive"]
        }
        SetNumber {
            name: "speed"
            description: "-1 is slowest, 1 fastest."
            hecateKey: "input.type:pointer.pointer_speed"
            fallback: 0
            from: -1
            to: 1
            stepSize: 0.1
        }
    }
    SetColumn {
        SetSection { text: "scrolling" }
        SetToggle {
            name: "natural scrolling"
            description: "Content follows your fingers, as on a phone."
            hecateKey: "input.type:pointer.natural_scroll"
            fallback: false
        }
    }
}
