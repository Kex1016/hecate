pragma ComponentBehavior: Bound
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland

// The `● rec` window of the Region board, up for as long as a recording runs: the time, what is
// recorded and a stop button.
PanelWindow {
    id: root

    // logical size of the region being recorded, "" when unknown (a recording from before the shell started)
    property string size: ""
    property int seconds: 0
    readonly property bool recording: Persistent.states.record.enable
    readonly property int topMargin: 56
    readonly property int heightWithGap: root.visible ? window.implicitHeight + transition.margin + 16 : 0

    function clock(total) {
        const pad = n => String(n).padStart(2, "0");
        const h = Math.floor(total / 3600);
        return (h > 0 ? pad(h) + ":" : "") + pad(Math.floor(total / 60) % 60) + ":" + pad(total % 60);
    }

    onRecordingChanged: {
        root.seconds = 0;
        if (!root.recording) root.size = "";
    }

    visible: transition.present
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "quickshell:regionRecord"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    anchors {
        top: true
        right: true
    }
    margins.top: root.topMargin - transition.margin
    margins.right: 40 - transition.margin
    implicitWidth: window.width + 2 * transition.margin
    implicitHeight: window.implicitHeight + 2 * transition.margin
    mask: Region { item: window }

    Timer {
        interval: 1000
        repeat: true
        running: root.recording
        onTriggered: root.seconds += 1
    }

    H.PixelTransition {
        id: transition
        x: transition.margin
        y: transition.margin
        width: window.width
        height: window.implicitHeight
        shown: root.recording

        H.HWindow {
            id: window
            width: 380
            height: implicitHeight
            label: "● rec"
            variant: "love"

            RowLayout {
                Layout.fillWidth: true
                spacing: 12

                H.HText {
                    ink: "c2"
                    text: root.clock(root.seconds)
                }
                H.HText {
                    role: "caption"
                    ink: "c1"
                    text: (root.size !== "" ? root.size + " · " : "")
                        + (Config.options.screenRecord.systemAudio ? "sound on" : "sound off") + " · "
                        + (Config.options.screenRecord.microphone ? "mic on" : "mic off")
                }
                Item { Layout.fillWidth: true }
                H.InkButton {
                    text: "stop"
                    ink: "c2"
                    onClicked: Quickshell.execDetached([Directories.recordScriptPath])
                }
            }
        }
    }
}
