import QtQuick
import qs.modules.ii.settings

SettingsPage {
    note: "saved to ~/.config/hecate/shell.toml"

    SetColumn {
        SetSection { text: "windows" }
        SetNumber {
            name: "gaps"
            description: "Between windows when snapping and maximising."
            hecateKey: "general.gaps"
            fallback: 0
            from: 0
            to: 64
            suffix: "px"
        }
        SetChoice {
            name: "new windows open"
            hecateKey: "placement.policy"
            fallback: "center"
            values: ["center", "cursor", "cascade", "automatic"]
            labels: ["centre", "cursor", "cascade", "auto"]
        }
        SetChoice {
            name: "title bars"
            description: "hecate: the pine frame with the window's label, no title bar. app: the app draws its own."
            hecateKey: "general.decorations"
            fallback: "server"
            values: ["server", "client"]
            labels: ["hecate", "app"]
        }
    }
    SetColumn {
        SetSection { text: "snapping" }
        SetToggle {
            name: "drag to the top to maximise"
            hecateKey: "snapping.top_maximize"
            fallback: true
        }
        SetNumber {
            name: "snap distance"
            hecateKey: "snapping.range.inner"
            fallback: 10
            from: 0
            to: 64
            suffix: "px"
        }
        SetNumber {
            name: "screen edge resistance"
            hecateKey: "resistance.screen_edge"
            fallback: 20
            from: 0
            to: 100
            stepSize: 5
            suffix: "px"
        }
        SetNote {
            text: "Frames are haiiro's: 5px double pine, square, with the window's label. They come from the Home Manager module, not from here."
        }
    }
}
