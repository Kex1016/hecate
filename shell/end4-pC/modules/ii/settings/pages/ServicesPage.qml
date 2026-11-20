import QtQuick
import qs.modules.common
import qs.modules.ii.settings
import qs.services.apple

SettingsPage {
    SetColumn {
        SetSection { text: "claude" }
        SetToggle {
            name: "claude"
            description: "Opt-in. Nothing is installed or sent until this is on, and it runs on your own Claude Code login."
            key: "claude.enable"
        }
        SetToggle {
            name: "sessions"
            description: "Claude Code sessions report here through their hooks, so you can answer them from the desktop."
            key: "claude.bridge.enable"
            enabled: Config.options.claude.enable
        }
        SetToggle {
            name: "requests in notifications"
            description: "Each permission request also goes to the notification list."
            key: "claude.bridge.notifications"
            enabled: Config.options.claude.enable && Config.options.claude.bridge.enable
        }
        SetSection { text: "apple" }
        SetText {
            name: "apple id"
            description: "Empty: the first iCloud account in your calendars."
            key: "apple.appleId"
            placeholder: Photos.appleId || "you@icloud.com"
            columns: 24
        }
        SetToggle {
            name: "airpods"
            description: "Battery and noise control in the bar and the quick panel, through librepods."
            key: "apple.airpods.enable"
        }
        SetToggle {
            name: "pause when one comes out"
            key: "apple.airpods.pauseOnRemove"
            enabled: Config.options.apple.airpods.enable
        }
        SetToggle {
            name: "icloud drive"
            description: "Mounted at login with rclone. Not available with Advanced Data Protection."
            key: "apple.drive.enable"
        }
        SetText {
            name: "mount at"
            key: "apple.drive.path"
            columns: 20
            enabled: Config.options.apple.drive.enable
        }
        SetAction {
            name: "sign in"
            description: Drive.status === "ok" ? "Drive is mounted." : Photos.status === "signIn" ? "Photos needs the password or a code."
                : "Opens a terminal for the password and the two-factor code."
            buttons: [{ text: "photos", ink: "c3" }, { text: "drive", ink: "c3" }]
            enabled: Photos.appleId !== ""
            onPressed: i => i === 0 ? Photos.signIn() : Drive.signIn()
        }
        SetNumber {
            name: "next album photo every"
            key: "apple.photos.rotate"
            from: 5
            to: 1440
            stepSize: 5
            suffix: "m"
        }
        SetNumber {
            name: "fetch the album every"
            key: "apple.photos.syncHours"
            from: 1
            to: 48
            suffix: "h"
        }
        SetToggle {
            name: "cider"
            description: "Library, lyrics and the queue from Cider's local RPC server, while Cider plays."
            key: "apple.cider.enable"
        }
        SetText {
            name: "cider token"
            description: "Cider's RPC token (Settings, Connectivity). Empty: the keyring's."
            key: "apple.cider.token"
            secret: true
            columns: 20
            enabled: Config.options.apple.cider.enable
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
