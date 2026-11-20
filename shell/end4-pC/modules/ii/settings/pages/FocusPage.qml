import QtQuick
import qs.modules.ii.settings

SettingsPage {
    note: "saved to ~/.config/hecate/shell.toml"

    SetColumn {
        SetSection { text: "focus" }
        SetToggle {
            name: "focus follows the mouse"
            hecateKey: "focus.follow_mouse"
            fallback: false
        }
        SetToggle {
            name: "raise windows when focused"
            hecateKey: "focus.raise_on_focus"
            fallback: true
        }
    }
    SetColumn {
        SetSection { text: "activation" }
        SetChoice {
            name: "apps may take focus"
            description: "smart: only when the request carries an activation token, like a link you just clicked."
            hecateKey: "focus.steal"
            fallback: "smart"
            values: ["never", "smart", "always"]
        }
    }
}
