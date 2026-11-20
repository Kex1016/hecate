pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro

// The keyboard layout's code ("us"). Hover: every layout, the current one
// marked, and a click on one picks it; click or wheel: the next one.
BarSegment {
    id: root

    readonly property string code: (WM.niriLike ? NiriXkb.currentLayoutCode : HyprlandXkb.currentLayoutCode) ?? ""
    readonly property var names: WM.niriLike ? NiriXkb.layoutNames : []

    visible: root.code.length > 0
    popupLabel: "kbd"
    popupWidth: 280
    onClicked: root.step("next")
    onWheel: event => root.step(event.angleDelta.y < 0 ? "next" : "prev")

    // "next", "prev" or a layout's index
    function step(layout) {
        const to = String(layout);
        if (WM.compositor === "hecate")
            Quickshell.execDetached(["hecate", "msg", "keyboard-layout", to]);
        else if (WM.compositor === "niri")
            Quickshell.execDetached(["niri", "msg", "action", "switch-layout", to]);
        else
            Quickshell.execDetached(["hyprctl", "switchxkblayout", "all", to]);
    }

    HText {
        text: root.code.toLowerCase()
    }

    popup: Component {
        ColumnLayout {
            spacing: 0

            Repeater {
                model: root.names

                ListRow {
                    required property string modelData
                    required property int index
                    Layout.fillWidth: true
                    text: modelData
                    selected: index === NiriXkb.currentIdx
                    onClicked: root.step(index)
                }
            }
            HText {
                visible: root.names.length === 0
                text: root.code.toLowerCase()
            }
        }
    }
}
