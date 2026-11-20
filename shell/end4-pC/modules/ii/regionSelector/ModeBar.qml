pragma ComponentBehavior: Bound
import qs
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts

// The mode bar of the Region board: `hecate: ~/region`, one ink button per mode (the current one
// lit), the options of that mode as chips on the right, and the keys in the footer.
H.HWindow {
    id: root

    property var action
    property var selectionMode
    property bool copyToClipboard: true
    property bool saveToFile: false

    signal actionPicked(var action)
    signal selectionModePicked(var mode)
    signal copyToggled()
    signal saveToggled()

    readonly property string modeId: RegionModes.idOf(root.action)
    readonly property bool annotate: root.action === RegionSelection.SnipAction.Edit
    readonly property bool recording: root.modeId === "record"
    readonly property bool soundOn: Config.options.screenRecord.systemAudio

    width: implicitWidth
    height: implicitHeight
    label: "hecate: ~/region"
    note: RegionModes.note(root.action)
    footer: [
        H.HText {
            role: "caption"
            ink: "inkMuted"
            text: RegionModes.description(root.action, root.selectionMode)
        },
        H.Keycap { key: "tab"; quiet: true; hint: "mode" },
        H.Keycap { key: "enter"; quiet: true; hint: "take" },
        H.Keycap { key: "esc"; quiet: true; hint: "cancel" }
    ]

    RowLayout {
        Layout.fillWidth: true
        spacing: 8

        Repeater {
            model: RegionModes.modes
            H.InkButton {
                required property var modelData
                readonly property bool current: modelData.id === root.modeId
                activeFocusOnTab: false
                text: modelData.id
                ink: current ? "c5" : ""
                flat: !current
                on: current
                onClicked: root.actionPicked(modelData.action)
            }
        }

        Item { Layout.fillWidth: true; Layout.minimumWidth: 16 }

        // what to take it from: a region (also what a click on a window gives), the screen, or a loop
        H.Segmented {
            visible: root.modeId !== "pick"
            activeFocusOnTab: false
            readonly property var targets: root.recording
                ? [RegionSelection.SelectionMode.RectCorners, RegionSelection.SelectionMode.ScreenTarget]
                : [RegionSelection.SelectionMode.RectCorners, RegionSelection.SelectionMode.ScreenTarget, RegionSelection.SelectionMode.Circle]
            model: root.recording ? ["region", "screen"] : ["region", "screen", "free"]
            currentIndex: Math.max(0, targets.indexOf(root.selectionMode))
            onActivated: index => root.selectionModePicked(targets[index])
        }

        H.Chip {
            visible: root.modeId === "shot"
            interactive: true
            activeFocusOnTab: false
            text: "copy"
            on: root.copyToClipboard
            onClicked: root.copyToggled()
        }
        H.Chip {
            visible: root.modeId === "shot"
            interactive: true
            activeFocusOnTab: false
            text: "save"
            on: root.saveToFile
            onClicked: root.saveToggled()
        }
        H.Chip {
            visible: root.modeId === "shot"
            interactive: true
            activeFocusOnTab: false
            text: "edit"
            on: root.annotate
            onClicked: root.actionPicked(root.annotate ? RegionSelection.SnipAction.Copy : RegionSelection.SnipAction.Edit)
        }
        H.Chip {
            visible: root.recording
            interactive: true
            activeFocusOnTab: false
            text: "sound"
            on: Config.options.screenRecord.systemAudio
            onClicked: Config.options.screenRecord.systemAudio = !Config.options.screenRecord.systemAudio
        }
        H.Chip {
            visible: root.recording
            interactive: true
            activeFocusOnTab: false
            text: "mic"
            on: Config.options.screenRecord.microphone
            onClicked: Config.options.screenRecord.microphone = !Config.options.screenRecord.microphone
        }
    }
}
