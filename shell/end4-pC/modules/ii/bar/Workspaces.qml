pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro

// The workspaces as numbers: the current one on a pine band, urgent ones
// love, empty ones muted. Holding Super spells out what is on each one
// ("2 zen ×3", "5 —"). Click switches, the wheel steps through them.
Item {
    id: root

    required property ShellScreen screen

    readonly property string output: root.screen?.name ?? ""
    readonly property int shown: Config.options.bar.workspaces.shown
    readonly property var activeId: WM.activeWorkspaceForMonitor(root.output)?.id ?? null

    // hecate and niri: the workspaces on this output (all of them in
    // hecate's global scope), in order, with what is on each
    readonly property var model: {
        const wins = WM.windowList ?? [];
        return (WM.workspaces ?? [])
            .filter(ws => ws.output === null || ws.output === undefined || ws.output === root.output)
            .slice()
            .sort((a, b) => (a.idx ?? a.id) - (b.idx ?? b.id))
            .slice(0, root.shown)
            .map(ws => {
                const here = wins.filter(w => w.workspaceId === ws.id);
                const top = here.find(w => w.focused)
                    ?? here.find(w => String(w.id) === String(ws.active_window_id))
                    ?? here.reduce((a, b) => (a === null || b.width * b.height > a.width * a.height ? b : a), null);
                const app = top ? BarState.appName(top.appId) : "";
                const same = top ? here.filter(w => BarState.appName(w.appId) === app).length : 0;
                return {
                    id: ws.id,
                    number: ws.idx ?? ws.id,
                    on: ws.id === root.activeId || (root.activeId === null && ws.is_active),
                    urgent: ws.is_urgent ?? false,
                    empty: here.length === 0,
                    app: top ? app + (same > 1 ? " ×" + same : "") : "—"
                };
            });
    }

    // held Super, after the delay that keeps a quick Mod+key from flashing it
    property bool spelled: false
    Timer {
        id: spellDelay
        interval: Config.options.bar.workspaces.showNumberDelay
        onTriggered: root.spelled = true
    }
    Connections {
        target: GlobalStates
        function onSuperDownChanged() {
            if (GlobalStates.superDown) {
                spellDelay.restart();
            } else {
                spellDelay.stop();
                root.spelled = false;
            }
        }
    }

    implicitWidth: strip.implicitWidth
    implicitHeight: strip.implicitHeight
    Layout.alignment: Qt.AlignVCenter

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: event => WM.switchWorkspaceRelative(event.angleDelta.y < 0 ? "next" : "prev")
    }

    Row {
        id: strip

        Repeater {
            model: root.model

            MouseArea {
                id: cell
                required property var modelData
                implicitWidth: label.implicitWidth + 2 * HTokens.ch()
                implicitHeight: label.implicitHeight
                cursorShape: Qt.PointingHandCursor
                onClicked: WM.switchWorkspace(cell.modelData.id)

                Rectangle {
                    anchors.fill: parent
                    visible: cell.modelData.on
                    color: HTokens.pine
                }

                HText {
                    id: label
                    x: HTokens.ch()
                    text: cell.modelData.number + (root.spelled ? " " + cell.modelData.app : "")
                    ink: cell.modelData.on ? "text" : cell.modelData.urgent ? "love" : cell.modelData.empty ? "muted" : "subtle"
                }
            }
        }
    }
}
