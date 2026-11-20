pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// The dashboard (Mod+Esc): one full-screen panel on the focused monitor, a
// heavy veil over the blobs, and the home, settings and (in wallpaper mode)
// wallpapers pages with the nav at the bottom. It pixelates in and out like
// every other panel.
Scope {
    id: root

    readonly property string focusedScreenName: WM.niriLike
        ? (WM.focusedMonitor?.name ?? "")
        : (Hyprland.focusedMonitor?.name ?? "")
    readonly property var focusedScreen: Quickshell.screens.find(s => s.name === root.focusedScreenName)
        ?? Quickshell.screens[0]

    Component.onCompleted: {
        GlobalStates.settingsOpen = false;
        // Reads shell.toml once, dropping frame and motion keys an older shell wrote there.
        if (WM.compositor === "hecate") HecateSettings.refresh();
    }

    // The launcher (and anything else) can open a page at a setting.
    Connections {
        target: GlobalStates
        function onSettingsTargetChanged() {
            const t = GlobalStates.settingsTarget;
            if (!t) return;
            GlobalStates.dashboardPage = 1;
            if (t.page) GlobalStates.dashboardGroup = t.page;
        }
    }

    LazyLoader {
        id: loader
        // Stays loaded through the close frames.
        active: GlobalStates.settingsOpen || (item?.present ?? false)

        PanelWindow {
            id: window
            readonly property bool present: transition.present

            screen: root.focusedScreen
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:settings"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: GlobalStates.settingsOpen ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            H.PixelTransition {
                id: transition
                anchors.fill: parent
                margin: 0
                shown: GlobalStates.settingsOpen

                DashboardContent {
                    anchors.fill: parent
                    open: GlobalStates.settingsOpen
                    onCloseRequested: GlobalStates.settingsOpen = false
                }
            }
        }
    }

    IpcHandler {
        target: "settings"
        function toggle(): void { GlobalStates.settingsOpen = !GlobalStates.settingsOpen; }
        function open(): void { GlobalStates.settingsOpen = true; }
        function close(): void { GlobalStates.settingsOpen = false; }
        // `qs ipc call settings page calendars`: open the dashboard on one settings page.
        function page(id: string): void {
            GlobalStates.dashboardPage = 1;
            GlobalStates.dashboardGroup = id;
            GlobalStates.settingsOpen = true;
        }
    }

    CompositorGlobalShortcut {
        name: "settingsToggle"
        description: "Toggles the dashboard"
        onPressed: GlobalStates.settingsOpen = !GlobalStates.settingsOpen
    }
}
