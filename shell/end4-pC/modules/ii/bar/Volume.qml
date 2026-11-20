pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Pipewire
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// "vol 64", the value in foam; muted shows "vol mute" in muted ink. Hover:
// output and input sliders; wheel: louder or quieter; click: the quick
// panel; middle click: mute.
BarSegment {
    id: root

    readonly property PwNode sink: Audio.sink
    readonly property PwNode source: Audio.source
    readonly property bool muted: root.sink?.audio?.muted ?? false
    readonly property int percent: Math.round((root.sink?.audio?.volume ?? 0) * 100)

    visible: root.sink !== null
    popupLabel: "vol"
    popupWidth: 320
    onClicked: mouse => {
        if (mouse.button === Qt.MiddleButton)
            Audio.toggleMute();
        else
            GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen;
    }
    onWheel: event => event.angleDelta.y > 0 ? Audio.incrementVolume() : Audio.decrementVolume()

    HText {
        text: "vol"
    }
    HText {
        text: root.muted ? "mute" : root.percent
        ink: root.muted ? "muted" : "foam"
    }

    function deviceName(node) {
        return String(node?.nickname || node?.description || node?.name || "").toLowerCase();
    }

    popup: Component {
        ColumnLayout {
            spacing: HTokens.space2

            HText {
                Layout.fillWidth: true
                text: root.deviceName(root.sink)
                ink: "inkMuted"
                role: "caption"
                elide: Text.ElideRight
            }
            Slider {
                Layout.fillWidth: true
                label: "out"
                value: root.sink?.audio?.volume ?? 0
                valueText: root.muted ? "mute" : root.percent
                onMoved: v => {
                    if (root.sink?.audio)
                        root.sink.audio.volume = v;
                }
            }
            HText {
                Layout.fillWidth: true
                Layout.topMargin: HTokens.space2
                visible: root.source !== null
                text: root.deviceName(root.source)
                ink: "inkMuted"
                role: "caption"
                elide: Text.ElideRight
            }
            Slider {
                Layout.fillWidth: true
                visible: root.source !== null
                label: "mic"
                value: root.source?.audio?.volume ?? 0
                valueText: (root.source?.audio?.muted ?? false) ? "mute" : Math.round((root.source?.audio?.volume ?? 0) * 100)
                onMoved: v => {
                    if (root.source?.audio)
                        root.source.audio.volume = v;
                }
            }
        }
    }
}
