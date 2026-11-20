import QtQuick
import qs.services
import qs.modules.common
import qs.modules.ii.settings

SettingsPage {
    SetColumn {
        SetSection { text: "you" }
        SetText {
            name: "name"
            description: "What the dashboard and the narrator call you."
            key: "profile.displayName"
            placeholder: SystemInfo.username
        }
        SetSection { text: "night light" }
        SetToggle {
            name: "automatic"
            description: "Warm the screens between the two times below."
            key: "light.night.automatic"
        }
        SetText {
            name: "from"
            key: "light.night.from"
            columns: 6
            enabled: Config.options.light.night.automatic
        }
        SetText {
            name: "to"
            key: "light.night.to"
            columns: 6
            enabled: Config.options.light.night.automatic
        }
        SetNumber {
            name: "warmth"
            key: "light.night.colorTemperature"
            from: 1000
            to: 6500
            stepSize: 250
            suffix: "K"
        }
    }
    SetColumn {
        SetSection { text: "battery" }
        SetNumber {
            name: "low"
            description: "A warning at this charge."
            key: "battery.low"
            from: 5
            to: 50
            suffix: "%"
        }
        SetNumber {
            name: "critical"
            key: "battery.critical"
            from: 1
            to: 25
            suffix: "%"
        }
        SetToggle {
            name: "suspend when critical"
            key: "battery.automaticSuspend"
        }
        SetToggle {
            name: "battery sounds"
            key: "sounds.battery"
        }
        SetSection { text: "audio" }
        SetToggle {
            name: "volume protection"
            description: "Holds back sudden jumps in volume."
            key: "audio.protection.enable"
        }
        SetNumber {
            name: "loudest"
            key: "audio.protection.maxAllowed"
            from: 10
            to: 100
            stepSize: 5
            suffix: "%"
            enabled: Config.options.audio.protection.enable
        }
    }
}
