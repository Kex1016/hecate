import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// The quick panel (Mod+N): one window, one layout, on the right under the bar.
// It pixelates in and out (PixelTransition); the layer stays mapped until the
// close frames have played.
Scope {
    id: root

    // QuickPanel board: 480px wide, 24px from the right, the bar and the bottom.
    readonly property int panelWidth: 480
    readonly property int gap: H.HTokens.space6

    Component.onCompleted: QuickState.gameMode // start the Game Mode state with the shell

    PanelWindow {
        id: panelWindow

        visible: transition.present

        function hide() {
            GlobalStates.sidebarRightOpen = false;
        }

        onVisibleChanged: {
            if (visible)
                GlobalFocusGrab.addDismissable(panelWindow);
            else
                GlobalFocusGrab.removeDismissable(panelWindow);
        }

        Connections {
            target: GlobalFocusGrab
            function onDismissed() {
                panelWindow.hide();
            }
        }

        Connections {
            target: GlobalStates
            function onSidebarRightOpenChanged() {
                if (!GlobalStates.sidebarRightOpen)
                    QuickState.view = "";
            }
            function onRequestBluetoothDialog() {
                if (BluetoothStatus.available)
                    QuickState.openView("bluetooth");
            }
        }

        Connections {
            target: Vpn
            function onDialogRequested() {
                QuickState.openView("vpn");
            }
        }

        // Exclusive zone 0: the layer keeps clear of the bar, so the bar stays usable.
        exclusiveZone: 0
        WlrLayershell.namespace: "quickshell:sidebarRight"
        WlrLayershell.keyboardFocus: GlobalStates.sidebarRightOpen ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        color: "transparent"

        anchors {
            top: true
            right: true
            bottom: true
            left: true
        }

        // a click beside the panel closes it
        MouseArea {
            anchors.fill: parent
            onClicked: panelWindow.hide()
        }

        H.PixelTransition {
            id: transition
            shown: GlobalStates.sidebarRightOpen
            anchors {
                top: parent.top
                bottom: parent.bottom
                right: parent.right
                margins: root.gap
            }
            width: root.panelWidth

            // keep clicks inside the panel from closing it
            MouseArea {
                anchors.fill: parent
                onClicked: mouse => mouse.accepted = true
            }

            Loader {
                id: content
                anchors.fill: parent
                active: transition.present
                focus: GlobalStates.sidebarRightOpen
                sourceComponent: QuickPanel {}

                Keys.onPressed: event => {
                    if (event.key !== Qt.Key_Escape)
                        return;
                    if (QuickState.view !== "")
                        QuickState.view = "";
                    else
                        panelWindow.hide();
                    event.accepted = true;
                }
            }
        }

        IpcHandler {
            target: "sidebarRight"

            function toggle(): void {
                GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen;
            }

            function close(): void {
                GlobalStates.sidebarRightOpen = false;
            }

            function open(): void {
                GlobalStates.sidebarRightOpen = true;
            }
        }

        CompositorGlobalShortcut {
            name: "sidebarRightToggle"
            description: "Toggles the quick panel"

            onPressed: GlobalStates.sidebarRightOpen = !GlobalStates.sidebarRightOpen
        }
        CompositorGlobalShortcut {
            name: "sidebarRightOpen"
            description: "Opens the quick panel"

            onPressed: GlobalStates.sidebarRightOpen = true
        }
        CompositorGlobalShortcut {
            name: "sidebarRightClose"
            description: "Closes the quick panel"

            onPressed: GlobalStates.sidebarRightOpen = false
        }
    }
}
