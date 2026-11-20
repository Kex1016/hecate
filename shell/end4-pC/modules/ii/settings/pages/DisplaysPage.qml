pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.ii.settings
import qs.modules.common.widgets.haiiro as H

// One column per screen hecate reports: mode, scale, rotation, variable
// refresh and position. Saved to ~/.config/hecate/shell.toml under
// [outputs.<name>].
SettingsPage {
    id: page

    note: "saved to ~/.config/hecate/shell.toml"

    Component.onCompleted: HecateSettings.refresh()

    SetColumn {
        visible: HecateSettings.outputs.length === 0
        SetSection { text: "displays" }
        SetNote {
            text: "No displays reported by hecate."
        }
    }

    Repeater {
        model: HecateSettings.outputs

        SetColumn {
            id: screen
            required property var modelData
            readonly property string base: `outputs.${modelData.name}`
            readonly property var modes: HecateSettings.modes(modelData)
            readonly property int modeIndex: Math.max(0, modes.findIndex(m => HecateSettings.modeValue(m) === HecateSettings.currentMode(modelData)))
            readonly property var position: HecateSettings.value(`${base}.position`, [modelData.logical?.x ?? 0, modelData.logical?.y ?? 0])

            SetSection {
                text: [screen.modelData.name, screen.modelData.make, screen.modelData.model].filter(s => s).join(" · ")
            }
            SetRow {
                name: "mode"
                H.Spin {
                    value: screen.modeIndex
                    from: 0
                    to: Math.max(0, screen.modes.length - 1)
                    // first is the largest and fastest, so → goes down the list
                    text: screen.modes.length > 0 ? HecateSettings.modeLabel(screen.modes[value]) : "default"
                    onMoved: v => HecateSettings.set(`${screen.base}.mode`, HecateSettings.modeValue(screen.modes[v]))
                }
            }
            SetNumber {
                name: "scale"
                hecateKey: `${screen.base}.scale`
                fallback: screen.modelData.logical?.scale ?? 1
                from: 0.5
                to: 3
                stepSize: 0.25
                suffix: "×"
            }
            SetChoice {
                name: "rotation"
                hecateKey: `${screen.base}.transform`
                fallback: screen.modelData.logical?.transform ?? "normal"
                values: ["normal", "90", "180", "270"]
                labels: ["0°", "90°", "180°", "270°"]
            }
            SetChoice {
                name: "variable refresh"
                description: "fullscreen: only while a fullscreen window is up."
                hecateKey: `${screen.base}.adaptive_sync`
                fallback: "off"
                values: ["off", "on", "fullscreen"]
            }
            SetRow {
                name: "position"
                description: "Logical pixels, top left of the screen."
                Row {
                    spacing: H.HTokens.space4
                    H.Spin {
                        value: screen.position[0]
                        from: -16384
                        to: 16384
                        stepSize: 10
                        text: "x " + value
                        onMoved: v => HecateSettings.set(`${screen.base}.position`, [v, screen.position[1]])
                    }
                    H.Spin {
                        value: screen.position[1]
                        from: -16384
                        to: 16384
                        stepSize: 10
                        text: "y " + value
                        onMoved: v => HecateSettings.set(`${screen.base}.position`, [screen.position[0], v])
                    }
                }
            }
        }
    }
}
