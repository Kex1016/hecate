pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets
import qs.modules.common.widgets.haiiro
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// The media panel (Mod+M, the `mediaControlsToggle` shortcut): the haiiro
// window `hecate: ~/media` under the bar, 80px from the left edge. It shows
// the player MprisController picks (Spotify or Cider, whichever played last,
// then a browser or mpv), and the chips in it pick another by hand. `e` opens
// the equalizer beside it.
Scope {
    id: root

    readonly property var players: MprisController.rankedPlayers
    readonly property var activePlayer: MprisController.activePlayer

    // where the window sits on the Media board: 80px from the left, a bar and
    // 40px down
    readonly property int marginLeft: HTokens.space20
    readonly property int marginTop: HTokens.barHeight + 40

    // the surface stays mapped until the close frames have played
    property bool keepAlive: false
    Connections {
        target: GlobalStates
        function onMediaControlsOpenChanged() {
            if (GlobalStates.mediaControlsOpen) root.keepAlive = true;
        }
    }

    Process {
        id: cavaProc
        // only while something shows the bars: this panel, the quick panel, the equalizer
        // or the bar's media hover
        running: (GlobalStates.mediaControlsOpen ||
            GlobalStates.sidebarRightOpen ||
            GlobalStates.equalizerOpen ||
            GlobalStates.barMediaHover)
            && (MprisController.activePlayer?.isPlaying ?? false)
        onRunningChanged: {
            if (!cavaProc.running) {
                GlobalStates.visualizerPoints = [];
            }
        }
        command: ["cava", "-p", `${FileUtils.trimFileProtocol(Directories.scriptPath)}/cava/raw_output_config.txt`]
        stdout: SplitParser {
            onRead: data => {
                let points = data.split(";").map(p => parseFloat(p.trim())).filter(p => !isNaN(p));
                GlobalStates.visualizerPoints = points;
            }
        }
    }

    Loader {
        id: mediaControlsLoader
        active: GlobalStates.mediaControlsOpen || root.keepAlive

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
            WlrLayershell.namespace: "quickshell:mediaControls"
            // the keys on the footer (space, arrows, tab, e) need the keyboard
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
                function onDismissed() {
                    GlobalStates.mediaControlsOpen = false;
                }
            }

            PixelTransition {
                id: transition
                x: panelWindow.pad
                y: panelWindow.pad
                shown: GlobalStates.mediaControlsOpen
                width: panel.implicitWidth
                height: panel.implicitHeight
                onClosed: root.keepAlive = false

                MediaPanel {
                    id: panel
                    width: transition.width
                    height: transition.height
                    player: root.activePlayer
                    players: root.players
                    onPlayerPicked: p => MprisController.setActivePlayer(p)
                    onEqualizerRequested: GlobalStates.equalizerOpen = !GlobalStates.equalizerOpen
                    onCloseRequested: GlobalStates.mediaControlsOpen = false
                    Component.onCompleted: forceActiveFocus()
                }
            }
        }
    }

    IpcHandler {
        target: "mediaControls"

        function toggle(): void {
            GlobalStates.mediaControlsOpen = !GlobalStates.mediaControlsOpen;
            if (GlobalStates.mediaControlsOpen)
                Notifications.timeoutAll();
        }

        function close(): void {
            GlobalStates.mediaControlsOpen = false;
        }

        function open(): void {
            GlobalStates.mediaControlsOpen = true;
            Notifications.timeoutAll();
        }
    }

    CompositorGlobalShortcut {
        name: "mediaControlsToggle"
        description: "Toggles media controls on press"

        onPressed: {
            GlobalStates.mediaControlsOpen = !GlobalStates.mediaControlsOpen;
        }
    }
    CompositorGlobalShortcut {
        name: "mediaControlsOpen"
        description: "Opens media controls on press"

        onPressed: {
            GlobalStates.mediaControlsOpen = true;
        }
    }
    CompositorGlobalShortcut {
        name: "mediaControlsClose"
        description: "Closes media controls on press"

        onPressed: {
            GlobalStates.mediaControlsOpen = false;
        }
    }
}
