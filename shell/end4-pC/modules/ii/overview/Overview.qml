pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// The launcher (Mod+A, the `searchToggle` shortcut). A veil over the
// desktop under the bar, the launcher window 160px from the top, and
// `man launch` under it while no prefix is in use. Mod+V and Mod+. open it
// in the clipboard and emoji modes. Tapping Super alone does nothing.
Scope {
    id: overviewScope

    PanelWindow {
        id: panelWindow
        readonly property bool open: GlobalStates.overviewOpen
        // stays mapped while the close frames play
        visible: open || windowFx.present || helpFx.present

        WlrLayershell.namespace: "quickshell:overview"
        WlrLayershell.layer: WlrLayer.Top
        WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"

        anchors {
            top: true
            bottom: true
            left: true
            right: true
        }

        // no input at all while closing
        mask: Region {
            item: panelWindow.open ? inputArea : null
        }
        Item {
            id: inputArea
            anchors.fill: parent
        }

        Connections {
            target: GlobalStates
            function onOverviewOpenChanged() {
                if (GlobalStates.overviewOpen) {
                    // opened from elsewhere (the bar's button): a fresh search
                    if (!overviewScope.keepQuery)
                        LauncherSearch.query = "";
                    overviewScope.keepQuery = false;
                    GlobalFocusGrab.addDismissable(panelWindow);
                    launcher.focusField();
                } else {
                    GlobalFocusGrab.dismiss();
                }
            }
        }

        Connections {
            target: GlobalFocusGrab
            function onDismissed() {
                GlobalStates.overviewOpen = false;
            }
        }

        H.Veil {
            id: veil
            visible: panelWindow.open
            anchors.fill: parent
            anchors.topMargin: H.HTokens.barHeight
            onClicked: GlobalStates.overviewOpen = false
        }

        Column {
            id: stack
            x: Math.round((parent.width - width) / 2)
            y: 160
            spacing: 40

            H.PixelTransition {
                id: windowFx
                shown: panelWindow.open

                Launcher {
                    id: launcher
                    listMax: Math.max(160, panelWindow.height - stack.y - 80 - (helpFx.shown ? help.height + stack.spacing : 0) - 200)
                    onDone: GlobalStates.overviewOpen = false
                }
            }

            H.PixelTransition {
                id: helpFx
                index: 1
                shown: panelWindow.open && LauncherSearch.mode === "search"

                LauncherHelp {
                    id: help
                }
            }
        }
    }

    // a fresh search each time it opens
    function toggle() {
        if (GlobalStates.overviewOpen)
            GlobalStates.overviewOpen = false;
        else
            overviewScope.openIn("search");
    }

    function openIn(mode) {
        // the same key again closes it
        if (GlobalStates.overviewOpen && LauncherSearch.mode === mode) {
            GlobalStates.overviewOpen = false;
            return;
        }
        overviewScope.show(mode === "search" ? "" : LauncherSearch.prefixes.find(p => p.mode === mode).token);
    }

    property bool keepQuery: false
    function show(query) {
        LauncherSearch.query = query;
        overviewScope.keepQuery = !GlobalStates.overviewOpen;
        GlobalStates.overviewOpen = true;
    }

    IpcHandler {
        target: "search"

        function toggle(): void {
            overviewScope.toggle();
        }
        function close(): void {
            GlobalStates.overviewOpen = false;
        }
        function open(): void {
            overviewScope.openIn("search");
        }
        // open with a query, prefix included: `qs ipc call search query "v "`
        function query(text: string): void {
            overviewScope.show(text);
        }
        function clipboardToggle(): void {
            overviewScope.openIn("clipboard");
        }
        function emojiToggle(): void {
            overviewScope.openIn("emoji");
        }
    }

    CompositorGlobalShortcut {
        name: "searchToggle"
        description: "Opens or closes the launcher (Mod+A)"
        onPressed: overviewScope.toggle()
    }
    CompositorGlobalShortcut {
        name: "overviewClipboardToggle"
        description: "Opens the launcher on the clipboard (Mod+V)"
        onPressed: overviewScope.openIn("clipboard")
    }
    CompositorGlobalShortcut {
        name: "overviewEmojiToggle"
        description: "Opens the launcher on the emoji picker (Mod+.)"
        onPressed: overviewScope.openIn("emoji")
    }
}
