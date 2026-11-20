pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// The equalizer window (`hecate: ~/media/eq`). It opens from the `e` key in
// the media panel (GlobalStates.equalizerOpen) and sits to the right of it,
// 80px away, the way the Media board has it. Like the media panel it is made
// by a Loader that keeps the surface mapped until the close frames have played.
Scope {
    id: root

    // the media panel's left margin and width, and the gap between them
    readonly property int marginLeft: HTokens.space20 + HTokens.panelWidth + HTokens.space20
    readonly property int marginTop: HTokens.barHeight + 40

    property bool keepAlive: false
    Connections {
        target: GlobalStates
        function onEqualizerOpenChanged() {
            if (GlobalStates.equalizerOpen) root.keepAlive = true;
        }
    }

    Loader {
        id: equalizerLoader
        active: GlobalStates.equalizerOpen || root.keepAlive

        sourceComponent: PanelWindow {
            id: panelWindow
            visible: true

            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0
            // room around the window for the notch label and the pixelate cells
            readonly property int pad: transition.margin
            implicitWidth: transition.width + 2 * pad
            implicitHeight: transition.height + 2 * pad
            color: "transparent"
            WlrLayershell.namespace: "quickshell:equalizer"
            // the preset name and the last.fm key are typed in here
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand

            anchors {
                top: true
                left: true
            }
            margins {
                top: root.marginTop - panelWindow.pad
                left: root.marginLeft - panelWindow.pad
            }

            mask: Region {
                item: transition
            }

            Component.onCompleted: GlobalFocusGrab.addDismissable(panelWindow)
            Component.onDestruction: GlobalFocusGrab.removeDismissable(panelWindow)
            Connections {
                target: GlobalFocusGrab
                function onDismissed() { GlobalStates.equalizerOpen = false }
            }

            PixelTransition {
                id: transition
                x: panelWindow.pad
                y: panelWindow.pad
                shown: GlobalStates.equalizerOpen
                width: eqView.implicitWidth
                height: eqView.implicitHeight
                onClosed: root.keepAlive = false

                EqualizerView {
                    id: eqView
                    width: transition.width
                    height: transition.height
                    onCloseRequested: GlobalStates.equalizerOpen = false
                    Component.onCompleted: forceActiveFocus()
                }
            }
        }
    }

    IpcHandler {
        target: "equalizer"

        function toggle(): void { GlobalStates.equalizerOpen = !GlobalStates.equalizerOpen; }
        function open(): void { GlobalStates.equalizerOpen = true; }
        function close(): void { GlobalStates.equalizerOpen = false; }
    }
}
