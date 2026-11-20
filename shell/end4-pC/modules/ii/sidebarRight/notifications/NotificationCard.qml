pragma ComponentBehavior: Bound
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Notifications

// One notification in the quick panel: a hairline box with the app in foam
// and how long ago, then sender: message, then its actions and dismiss.
// Low urgency is dashed, critical is love. Click runs the default action;
// right or middle click dismisses.
Item {
    id: root

    required property var notif
    readonly property bool critical: root.notif.urgency == NotificationUrgency.Critical
    readonly property bool low: root.notif.urgency == NotificationUrgency.Low
    readonly property var actions: (root.notif.actions ?? []).filter(a => a.identifier !== "default")
    readonly property bool hasDefault: (root.notif.actions ?? []).some(a => a.identifier === "default")

    // "&lt;b&gt;x&lt;/b&gt;" and <b>x</b> both come out as plain x
    function plain(s) {
        return (s ?? "").replace(/<[^>]*>/g, "").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"").replace(/&#39;/g, "'").replace(/&amp;/g, "&").trim();
    }
    readonly property string summary: plain(root.notif.summary)
    readonly property string body: plain(root.notif.body)
    // a body that is only a path (a saved screenshot) reads as a file, in iris
    readonly property bool bodyIsPath: /^(~|\/)[^\s]*$/.test(root.body)

    function ago(t, now) {
        const s = Math.max(0, Math.floor((now - t) / 1000));
        if (s < 60) return "now";
        if (s < 3600) return Math.floor(s / 60) + "m";
        if (s < 86400) return Math.floor(s / 3600) + "h";
        return Math.floor(s / 86400) + "d";
    }

    implicitHeight: box.implicitHeight + 16

    H.HDashFrame {
        anchors.fill: parent
        dashed: root.low && !root.critical
        color: root.critical ? H.HTokens.love : H.HTokens.inkHairline
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        cursorShape: root.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: mouse => {
            if (mouse.button === Qt.LeftButton) {
                if (!root.hasDefault)
                    return;
                Notifications.attemptInvokeAction(root.notif.notificationId, "default");
                GlobalStates.sidebarRightOpen = false;
            } else {
                Notifications.discardNotification(root.notif.notificationId);
            }
        }
    }

    ColumnLayout {
        id: box
        x: 10
        y: 8
        width: parent.width - 20
        spacing: H.HTokens.space1

        RowLayout {
            Layout.fillWidth: true
            H.HText {
                Layout.fillWidth: true
                text: (root.notif.appName || "notification").toLowerCase()
                ink: root.critical ? "c2" : "c6"
                elide: Text.ElideRight
            }
            H.HText {
                text: root.ago(root.notif.time, DateTime.clock.date.getTime())
                role: "caption"
                ink: "c1"
            }
        }

        // sender: message, or whichever of the two there is
        Text {
            Layout.fillWidth: true
            visible: text.length > 0
            readonly property var s: H.HTokens.role("body")
            function esc(t) {
                return t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
            }
            text: {
                const sum = root.summary, body = root.body;
                const bodyInk = root.bodyIsPath ? H.HTokens.iris : H.HTokens.text;
                const b = body.length > 0 ? `<font color="${bodyInk}">${esc(body)}</font>` : "";
                if (sum.length === 0) return b;
                if (body.length === 0) return esc(sum);
                return `<font color="${H.HTokens.pine}">${esc(sum)}</font>: ${b}`;
            }
            textFormat: Text.StyledText
            color: H.HTokens.text
            wrapMode: Text.Wrap
            maximumLineCount: 4
            elide: Text.ElideRight
            renderType: Text.NativeRendering
            lineHeightMode: Text.FixedHeight
            lineHeight: H.HTokens.lineBox(s.size)
            font.family: H.HTokens.mono
            font.pixelSize: s.size
            font.kerning: false
            font.hintingPreference: Font.PreferFullHinting
        }

        Flow {
            visible: root.actions.length > 0
            Layout.fillWidth: true
            spacing: H.HTokens.space2

            Repeater {
                model: root.actions
                H.InkButton {
                    required property var modelData
                    required property int index
                    // the first action is the one that acts
                    ink: index === 0 ? "c3" : ""
                    flat: index > 0
                    text: (modelData.text || modelData.identifier).toLowerCase()
                    onClicked: Notifications.attemptInvokeAction(root.notif.notificationId, modelData.identifier)
                }
            }
            H.InkButton {
                ink: "c1"
                text: "dismiss"
                onClicked: Notifications.discardNotification(root.notif.notificationId)
            }
        }
    }
}
