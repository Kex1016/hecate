pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

/**
 * The polkit agent's dialog: a gold window over the heavy veil, on the focused monitor.
 * Anything asking for privilege wears the gold frame.
 */
Scope {
    id: root

    // Mapped while the agent has a request, and until the close frames have played.
    property bool mapped: false

    Connections {
        target: PolkitService
        function onActiveChanged() {
            if (PolkitService.active)
                root.mapped = true;
            else if (H.HTokens.cut)
                root.mapped = false;
        }
    }

    Loader {
        active: root.mapped
        sourceComponent: PanelWindow {
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:polkit"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "transparent"
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }
            margins.top: H.HTokens.barHeight

            H.Veil {
                anchors.fill: parent
                heavy: true
                // a click outside does not answer for you
                onClicked: {}
            }

            PolkitContent {
                anchors.fill: parent
                shown: PolkitService.active
                onClosed: if (!PolkitService.active) root.mapped = false
            }
        }
    }
}
