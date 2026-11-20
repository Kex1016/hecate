import QtQuick
import qs.modules.common
import qs.modules.ii.settings

SettingsPage {
    SetColumn {
        SetSection { text: "claude" }
        SetToggle {
            name: "claude"
            description: "Opt-in. Nothing is installed or sent until this is on, and it runs on your own Claude Code login."
            key: "claude.enable"
        }
        SetSection { text: "weather" }
        SetToggle {
            name: "weather"
            description: "The clock's hover, the dashboard and the weather widget."
            key: "bar.weather.enable"
        }
        SetToggle {
            name: "use my location"
            key: "bar.weather.enableGPS"
            enabled: Config.options.bar.weather.enable
        }
        SetText {
            name: "city"
            description: "When the location is off."
            key: "bar.weather.city"
            columns: 20
            enabled: Config.options.bar.weather.enable && !Config.options.bar.weather.enableGPS
        }
        SetChoice {
            name: "units"
            key: "bar.weather.useUSCS"
            values: [false, true]
            labels: ["metric", "imperial"]
            enabled: Config.options.bar.weather.enable
        }
        SetNumber {
            name: "refresh every"
            key: "bar.weather.fetchInterval"
            from: 5
            to: 120
            stepSize: 5
            suffix: "m"
            enabled: Config.options.bar.weather.enable
        }
    }
    SetColumn {
        SetSection { text: "updates" }
        SetToggle {
            name: "check for updates"
            key: "updates.enableCheck"
        }
        SetNumber {
            name: "check every"
            key: "updates.checkInterval"
            from: 30
            to: 1440
            stepSize: 30
            suffix: "m"
            enabled: Config.options.updates.enableCheck
        }
        SetSection { text: "other daemons" }
        SetToggle {
            name: "stop other notification daemons"
            description: "So toasts only come from the shell."
            key: "conflictKiller.autoKillNotificationDaemons"
        }
        SetToggle {
            name: "stop other trays"
            key: "conflictKiller.autoKillTrays"
        }
        SetSection { text: "music recognition" }
        SetNumber {
            name: "listen for"
            key: "musicRecognition.timeout"
            from: 4
            to: 60
            stepSize: 2
            suffix: "s"
        }
    }
}
