import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets
import qs.modules.common.functions
import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import qs.modules.common.widgets.haiiro as H

Scope {
    id: root

    property bool reallyOpen: false

    Connections {
        target: GlobalStates
        function onWallpaperSelectorOpenChanged() {
            if (GlobalStates.wallpaperSelectorOpen) {
                closeAnimTimer.stop();
                root.reallyOpen = true;
            } else {
                closeAnimTimer.restart();
            }
        }
    }

    Timer {
        id: closeAnimTimer
        interval: H.HTokens.pixelateDuration + 50
        onTriggered: root.reallyOpen = false
    }

    Loader {
        id: wallpaperSelectorLoader
        active: root.reallyOpen && Wallpapers.enabled

        sourceComponent: PanelWindow {
            id: panelWindow
            readonly property var monitor: WM.monitorFor(panelWindow.screen)
            property bool monitorIsFocused: WM.focusedMonitor?.name == monitor?.name

            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:wallpaperSelector"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
            color: "transparent"

            // only on the monitor with focus
            visible: monitorIsFocused

            mask: Region { item: pixel }

            implicitWidth: content.implicitWidth + 2 * pixel.margin
            implicitHeight: content.implicitHeight + 2 * pixel.margin

            Component.onCompleted: GlobalFocusGrab.addDismissable(panelWindow)
            Component.onDestruction: GlobalFocusGrab.removeDismissable(panelWindow)
            Connections {
                target: GlobalFocusGrab
                function onDismissed() { GlobalStates.wallpaperSelectorOpen = false; }
            }

            H.PixelTransition {
                id: pixel
                anchors.centerIn: parent
                shown: GlobalStates.wallpaperSelectorOpen

                WallpaperSelectorContent {
                    id: content
                    monitorIsFocused: panelWindow.monitorIsFocused
                }
            }
        }
    }

    function toggleWallpaperSelector() {
        // wallpapers are opt-in: no picker until Settings > desktop > background says so
        if (!Wallpapers.enabled) return;
        GlobalStates.wallpaperSelectorOpen = !GlobalStates.wallpaperSelectorOpen
    }

    IpcHandler {
        target: "wallpaperSelector"

        function toggle(): void {
            root.toggleWallpaperSelector();
        }

        function random(): void {
            Wallpapers.randomFromCurrentFolder();
        }
    }

    CompositorGlobalShortcut {
        name: "wallpaperSelectorToggle"
        description: "Toggle wallpaper selector"
        onPressed: {
            root.toggleWallpaperSelector();
        }
    }

    CompositorGlobalShortcut {
        name: "wallpaperSelectorRandom"
        description: "Select random wallpaper in current folder"
        onPressed: {
            Wallpapers.randomFromCurrentFolder();
        }
    }
}