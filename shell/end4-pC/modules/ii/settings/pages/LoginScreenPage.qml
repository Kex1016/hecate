import QtQuick
import qs.modules.common
import qs.modules.ii.settings

SettingsPage {
    SetColumn {
        SetSection { text: "login screen" }
        SetToggle {
            name: "share with the login screen"
            description: "hecate-greetd always shows the blobs. Your keyboard and monitor layout can come along."
            key: "greeter.share"
        }
        SetToggle {
            name: "monitor layout"
            key: "greeter.displays"
            enabled: Config.options.greeter.share
        }
        SetToggle {
            name: "keyboard and mouse"
            key: "greeter.input"
            enabled: Config.options.greeter.share
        }
    }
    SetColumn {
        SetSection { text: "status line" }
        SetToggle {
            name: "status line"
            description: "The line under the clock."
            key: "greeter.showStatusText"
        }
        SetText {
            name: "status text"
            description: "Empty: Locked, like the lock screen."
            key: "greeter.statusText"
            placeholder: "Locked"
            columns: 24
            enabled: Config.options.greeter.showStatusText
        }
    }
}
