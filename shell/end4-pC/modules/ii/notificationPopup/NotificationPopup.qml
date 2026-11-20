pragma ComponentBehavior: Bound
import qs
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import qs.services
import qs.modules.ii.claude
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland

// Toasts: a stack of notify windows under the bar, top right, 420px wide.
// At most three show; the rest are counted under the stack. Critical ones
// (love frame) and permission requests (gold frame) stay until you answer
// them; everything else follows the service's timeout.
//
// Muting: do-not-disturb (Notifications.silent) and a fullscreen window on
// this monitor (notifications.silentInFullscreen) hold back everything except
// critical notifications and permission requests. Muted notifications are
// not lost, they wait in the quick panel.
Scope {
    id: notificationPopup

    readonly property int maxShown: 3
    readonly property int pad: 12 // room for the pixelate margin around each window

    property var screenName: WM.focusedMonitor?.name ?? ""
    readonly property bool fullscreen: WM.fullscreenOnMonitor(screenName)
    readonly property bool muted: Notifications.silent || (fullscreen && (Config.options.notifications.silentInFullscreen ?? true))

    function urgent(n) {
        return Toasts.isCritical(n.urgency) || Toasts.isPermission(n.appName, n.summary, n.notification?.hints?.category ?? "");
    }

    // what should be on screen now: urgent first, then newest first
    readonly property var wanted: Notifications.popupList
        .filter(n => notificationPopup.urgent(n) || !notificationPopup.muted)
        .sort((a, b) => (notificationPopup.urgent(b) - notificationPopup.urgent(a)) || (b.time - a.time))
    readonly property var shownIds: wanted.slice(0, maxShown).map(n => n.notificationId)
    readonly property int more: Math.max(0, wanted.length - maxShown)

    onShownIdsChanged: sync()
    Component.onCompleted: sync()

    ListModel {
        id: entries
    }

    function indexOfId(id) {
        for (let i = 0; i < entries.count; i++)
            if (entries.get(i).nid === id) return i;
        return -1;
    }

    function sync() {
        const ids = shownIds;
        for (let i = 0; i < entries.count; i++)
            entries.setProperty(i, "shown", ids.includes(entries.get(i).nid));
        // queue new ones at the top, keeping the order of `wanted`
        for (let k = ids.length - 1; k >= 0; k--) {
            const n = wanted.find(w => w.notificationId === ids[k]);
            const i = indexOfId(ids[k]);
            if (i !== -1 || !n) continue;
            const v = n.notification?.hints?.value;
            entries.insert(0, {
                nid: n.notificationId,
                shown: true,
                appName: n.appName,
                summary: n.summary,
                body: n.body,
                image: n.image,
                urgency: n.urgency,
                time: n.time,
                actionsJson: JSON.stringify(n.actions),
                progress: v !== undefined && v !== null ? Number(v) / 100 : -1
            });
        }
        reorder();
    }

    function reorder() {
        // urgent first, then newest; windows that are closing keep the tail
        const ids = shownIds;
        for (let p = 0; p < ids.length; p++) {
            const i = indexOfId(ids[p]);
            if (i > p) entries.move(i, p, 1);
        }
    }

    function drop(id) {
        const i = indexOfId(id);
        if (i !== -1 && !entries.get(i).shown) entries.remove(i);
    }

    // critical and permission toasts do not time out
    Connections {
        target: Notifications
        function onNotify(notif) {
            if (notificationPopup.urgent(notif)) Notifications.cancelTimeout(notif.notificationId);
        }
    }

    PanelWindow {
        id: root
        visible: entries.count > 0 && !GlobalStates.screenLocked
        screen: Quickshell.screens.find(s => s.name === notificationPopup.screenName) ?? null

        WlrLayershell.namespace: "quickshell:notificationPopup"
        WlrLayershell.layer: WlrLayer.Overlay
        exclusiveZone: 0

        anchors {
            top: true
            right: true
        }
        margins {
            // under the claude toasts, which start at the same corner (0 when none shows)
            top: H.HTokens.barHeight + 32 + SessionsState.toastsHeight - notificationPopup.pad
            right: 24 - notificationPopup.pad
        }

        color: "transparent"
        implicitWidth: stack.width + 2 * notificationPopup.pad
        implicitHeight: stack.height + 2 * notificationPopup.pad
        mask: Region {
            item: stack
        }

        Column {
            id: stack
            x: notificationPopup.pad
            y: notificationPopup.pad
            width: H.HTokens.toastWidth
            spacing: H.HTokens.toastGap

            Repeater {
                model: entries
                Toast {
                    required property var model
                    nid: model.nid
                    shown: model.shown
                    appName: model.appName
                    summary: model.summary
                    body: model.body
                    image: model.image
                    urgency: model.urgency
                    time: model.time
                    actionsJson: model.actionsJson
                    progress: model.progress
                    onClosed: notificationPopup.drop(model.nid)
                }
            }

            H.HText {
                visible: notificationPopup.more > 0
                width: parent.width
                horizontalAlignment: Text.AlignRight
                text: "+" + notificationPopup.more + " more in the quick panel (Mod+N)"
                role: "caption"
                ink: "c1"
            }
        }
    }
}
