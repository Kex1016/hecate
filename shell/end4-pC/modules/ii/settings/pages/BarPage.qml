import QtQuick
import qs.modules.common
import qs.modules.ii.settings

SettingsPage {
    SetColumn {
        SetSection { text: "visibility" }
        SetToggle {
            name: "hide when not in use"
            description: "The bar slides away and comes back at the top edge. Mod+J hides and shows it either way."
            key: "bar.autoHide.enable"
        }
        SetToggle {
            name: "show while holding super"
            key: "bar.autoHide.showWhenPressingSuper.enable"
            enabled: Config.options.bar.autoHide.enable
        }
        SetSection { text: "workspaces" }
        SetNumber {
            name: "shown"
            key: "bar.workspaces.shown"
            from: 1
            to: 20
        }
        SetNumber {
            name: "names after holding super"
            description: "Holding Super names each workspace by its top app after this long."
            key: "bar.workspaces.showNumberDelay"
            from: 0
            to: 1000
            stepSize: 50
            suffix: "ms"
        }
    }
    SetColumn {
        SetSection { text: "warnings" }
        SetNumber {
            name: "cpu"
            description: "The bar's meters turn love above these."
            key: "bar.resources.cpuWarningThreshold"
            from: 50
            to: 100
            stepSize: 5
            suffix: "%"
        }
        SetNumber {
            name: "memory"
            key: "bar.resources.memoryWarningThreshold"
            from: 50
            to: 100
            stepSize: 5
            suffix: "%"
        }
        SetNumber {
            name: "swap"
            key: "bar.resources.swapWarningThreshold"
            from: 50
            to: 100
            stepSize: 5
            suffix: "%"
        }
        SetSection { text: "tray" }
        SetToggle {
            name: "monochrome icons"
            key: "tray.monochromeIcons"
        }
        SetToggle {
            name: "hide passive items"
            description: "Apps that only sit there idle stay out of the tray."
            key: "tray.filterPassive"
        }
    }
}
