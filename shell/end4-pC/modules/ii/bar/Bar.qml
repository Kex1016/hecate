pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common.widgets.haiiro

// The one bar: 40px at the top of every screen, the same fixed segments
// on each. Mod+J hides and shows it; nothing else about it is a setting.
Scope {
    id: bar

    Variants {
        model: Quickshell.screens

        LazyLoader {
            id: barLoader
            required property ShellScreen modelData
            active: GlobalStates.barOpen && !GlobalStates.screenLocked && !GlobalStates.startupLockPending

            component: PanelWindow {
                id: barRoot
                screen: barLoader.modelData

                WlrLayershell.namespace: "quickshell:bar"
                WlrLayershell.layer: WlrLayer.Top
                exclusiveZone: HTokens.barHeight
                implicitHeight: HTokens.barHeight
                color: HTokens.surface

                anchors {
                    top: true
                    left: true
                    right: true
                }

                // part of the shared focus grab, so clicking the bar does not
                // dismiss the panel it opens
                Component.onCompleted: GlobalFocusGrab.addPersistent(barRoot)
                Component.onDestruction: GlobalFocusGrab.removePersistent(barRoot)

                BarContent {
                    anchors.fill: parent
                    screen: barLoader.modelData
                }
            }
        }
    }

    IpcHandler {
        target: "bar"

        function toggle(): void {
            GlobalStates.barOpen = !GlobalStates.barOpen;
        }

        function close(): void {
            GlobalStates.barOpen = false;
        }

        function open(): void {
            GlobalStates.barOpen = true;
        }
    }

    CompositorGlobalShortcut {
        name: "barToggle"
        description: "Toggles bar on press"
        onPressed: GlobalStates.barOpen = !GlobalStates.barOpen
    }

    CompositorGlobalShortcut {
        name: "barOpen"
        description: "Opens bar on press"
        onPressed: GlobalStates.barOpen = true
    }

    CompositorGlobalShortcut {
        name: "barClose"
        description: "Closes bar on press"
        onPressed: GlobalStates.barOpen = false
    }
}
