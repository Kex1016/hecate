pragma ComponentBehavior: Bound
import qs.services
import qs.services.claude
import qs.modules.common.widgets.haiiro as H
import QtQuick
import Quickshell
import Quickshell.Wayland

// Ask Claude (Mod+/, the `claudeAskToggle` shortcut; ClaudeAsk board).
// A 640px window at the top right of the focused screen, next to the window
// it asks about rather than over it, so there is no veil. The context it
// may send is taken when it opens (ClaudeAsk.openWith) and shown as chips.
Scope {
    id: root

    PanelWindow {
        id: panelWindow

        readonly property bool open: ClaudeAsk.open
        readonly property int pad: fx.margin

        screen: ClaudeAsk.screen
        // stays mapped while the close frames play
        visible: open || fx.present

        WlrLayershell.namespace: "quickshell:claudeAsk"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"

        anchors {
            top: true
            right: true
        }
        // the board: 50px under the bar, 60px from the right edge
        margins {
            top: H.HTokens.barHeight + 50 - pad
            right: 60 - pad
        }

        implicitWidth: H.HTokens.panelWidth + 2 * pad
        implicitHeight: Math.max(1, fx.implicitHeight) + 2 * pad

        mask: Region {
            item: panelWindow.open ? fx : null
        }

        Connections {
            target: ClaudeAsk
            function onOpenChanged() {
                if (ClaudeAsk.open)
                    GlobalFocusGrab.addDismissable(panelWindow);
                else
                    GlobalFocusGrab.removeDismissable(panelWindow);
            }
        }
        Connections {
            target: GlobalFocusGrab
            function onDismissed() {
                if (ClaudeAsk.open)
                    ClaudeAsk.close();
            }
        }

        H.PixelTransition {
            id: fx
            x: panelWindow.pad
            y: panelWindow.pad
            shown: panelWindow.open

            AskWindow {
                width: H.HTokens.panelWidth
                // room for the answer: the screen less the top margin and some air
                maxHeight: (panelWindow.screen?.height ?? 900) - H.HTokens.barHeight - 50 - 80
            }
        }
    }

    CompositorGlobalShortcut {
        name: "claudeAskToggle"
        description: "Asks Claude about the focused window (Mod+/)"
        onPressed: ClaudeAsk.toggle()
    }
}
