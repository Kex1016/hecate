pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.services.claude
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import Quickshell
import Quickshell.Wayland

// notify: nushell, the toast for a long command that failed (ClaudeFailToastWidget).
// The shell's nushell hook writes failed-command.json; this shows it top right, under the
// claude toasts and above the notification stack, until it is dismissed (the quick panel
// hides it for a while, like the other toasts). Nothing goes to
// Claude before its `ask claude` press. Hidden while the screen is locked or a fullscreen
// window is on that monitor.
Scope {
    id: root

    readonly property string screenName: WM.focusedMonitor?.name ?? ""
    readonly property bool present: widget.item?.present ?? false
    // out of the way while the quick panel, the ask panel or a card sits over its corner
    readonly property bool shown: root.present && !GlobalStates.screenLocked && !GlobalStates.sidebarRightOpen
        && !ClaudeAsk.open && !ClaudeAsk.cardOpen
        && !WM.fullscreenOnMonitor(root.screenName)

    onShownChanged: SessionsState.failToastHeight = root.shown ? fx.implicitHeight + 22 : 0

    PanelWindow {
        id: panel

        readonly property int pad: fx.margin

        screen: Quickshell.screens.find(s => s.name === root.screenName) ?? Quickshell.screens[0]
        visible: root.shown || fx.present

        WlrLayershell.namespace: "quickshell:claudeToasts"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"

        anchors {
            top: true
            right: true
        }
        margins {
            top: H.HTokens.barHeight + 32 + SessionsState.toastsHeight - pad
            right: 24 - pad
        }

        implicitWidth: Math.max(1, fx.implicitWidth) + 2 * pad
        implicitHeight: Math.max(1, fx.implicitHeight) + 2 * pad
        mask: Region {
            item: root.shown ? fx : null
        }

        H.PixelTransition {
            id: fx
            x: panel.pad
            y: panel.pad
            shown: root.shown
            onImplicitHeightChanged: if (root.shown) SessionsState.failToastHeight = implicitHeight + 22

            Loader {
                id: widget
                // the widget watches the file itself, so it stays loaded
                source: Qt.resolvedUrl("../background/widgets/ClaudeFailToastWidget.qml")
            }
        }
    }
}
