import QtQuick
import qs.modules.ii.settings

SettingsPage {
    note: "saved to ~/.config/hecate/shell.toml and config.json"

    SetColumn {
        SetSection { text: "motion" }
        SetToggle {
            name: "reduced motion"
            description: "Every transition is a cut and the background holds one still frame. Game Mode does the same while it is on."
            key: "motion.reduced"
        }
        SetNote {
            text: "Windows and panels pixelate in and out, and frames snap from colour to colour. That is haiiro's motion, set in the Home Manager module rather than here."
        }
    }
    SetColumn {
        SetSection { text: "fullscreen" }
        SetChoice {
            name: "tearing"
            description: "Lets games skip waiting for the next frame. fullscreen: only for fullscreen windows that ask."
            hecateKey: "gaming.allow_tearing"
            fallback: "fullscreen"
            values: ["never", "fullscreen", "always"]
        }
        SetToggle {
            name: "direct scanout"
            description: "A fullscreen window goes straight to the screen, skipping composition."
            hecateKey: "gaming.direct_scanout"
            fallback: true
        }
        SetToggle {
            name: "fullscreen keeps the screen awake"
            hecateKey: "gaming.fullscreen_inhibits_idle"
            fallback: true
        }
    }
}
