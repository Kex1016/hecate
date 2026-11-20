import QtQuick
import qs.modules.ii.settings

SettingsPage {
    SetColumn {
        SetSection { text: "idle" }
        SetNumber {
            name: "lock the screen after"
            description: "Apps that keep the screen awake (videos, games) and the keep-awake toggle hold these off."
            key: "hyprland.idle.lock"
            from: 0
            to: 240
            divisor: 60
            suffix: "m"
            zeroText: "never"
        }
        SetNumber {
            name: "turn the screens off after"
            key: "hyprland.idle.screenOff"
            from: 0
            to: 240
            divisor: 60
            suffix: "m"
            zeroText: "never"
        }
        SetNumber {
            name: "sleep after"
            key: "hyprland.idle.suspend"
            from: 0
            to: 720
            stepSize: 15
            divisor: 60
            suffix: "m"
            zeroText: "never"
        }
    }
    SetColumn {
        SetSection { text: "lock" }
        SetToggle {
            name: "unlock the keyring"
            description: "Your password at the lock screen also opens the keyring."
            key: "lock.security.unlockKeyring"
        }
        SetToggle {
            name: "password to power off"
            description: "The lock screen's power buttons ask first."
            key: "lock.security.requirePasswordToPower"
        }
    }
}
