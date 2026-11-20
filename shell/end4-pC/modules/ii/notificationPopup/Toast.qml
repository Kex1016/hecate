pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// One toast: a notify window in the kit. Frame colour says what it is:
// pine for anything, love for critical (stays until dismissed), gold for a
// permission request (stays until answered). The window pixelates in and out
// with H.PixelTransition; `closed` fires once the close frames have played.
H.PixelTransition {
    id: root

    required property int nid
    // snapshot taken when the toast was queued, so the close frames still
    // have something to draw after the service has dropped the notification
    required property string appName
    required property string summary
    required property string body
    required property string image
    required property string urgency
    required property double time
    required property string actionsJson
    required property real progress

    readonly property var live: Notifications.list.find(n => n.notificationId === root.nid) ?? null
    readonly property bool critical: Toasts.isCritical(live?.urgency ?? urgency)
    readonly property bool permission: Toasts.isPermission(appName, summary, live?.notification?.hints?.category ?? "")
    readonly property string variant: permission ? "gold" : critical ? "love" : "pine"
    readonly property var actions: live?.actions ?? JSON.parse(actionsJson)
    readonly property var buttons: actions.filter(a => a.identifier !== "default")
    readonly property string bodyText: Toasts.plain(live?.body ?? body)
    readonly property string title: live?.summary ?? summary
    readonly property real value: {
        const v = live?.notification?.hints?.value;
        return v !== undefined && v !== null ? Number(v) / 100 : progress;
    }
    readonly property string label: critical ? "notify: critical" : "notify: " + (appName || "system").toLowerCase()
    readonly property color titleInk: permission ? H.HTokens.gold : critical ? H.HTokens.love : H.HTokens.pine

    width: H.HTokens.toastWidth
    height: window.implicitHeight

    function buttonInk(a, i) {
        if (/^(deny|refuse|reject|no|block)/i.test(a.text)) return "c2";
        if (i === 0 && (permission || critical)) return "c3";
        return permission ? "c5" : "c1";
    }

    H.HWindow {
        id: window
        width: root.width
        label: root.label
        variant: root.variant

        MouseArea {
            id: whole
            Layout.fillWidth: true
            Layout.preferredHeight: col.implicitHeight
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton)
                    Notifications.discardNotification(root.nid);
                else if (root.critical || root.permission)
                    return;
                else if (root.actions.some(a => a.identifier === "default"))
                    Notifications.attemptInvokeAction(root.nid, "default");
                else
                    Notifications.discardNotification(root.nid);
            }

            ColumnLayout {
                id: col
                width: parent.width
                spacing: 6

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12

                    Item {
                        visible: root.image.length > 0
                        Layout.preferredWidth: 48
                        Layout.preferredHeight: 48
                        Layout.alignment: Qt.AlignTop
                        H.Dither { anchors.fill: parent; tone: "pine" }
                        Image {
                            anchors.fill: parent
                            source: root.image
                            sourceSize: Qt.size(48, 48)
                            fillMode: Image.PreserveAspectCrop
                            smooth: false
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        RowLayout {
                            Layout.fillWidth: true
                            H.HText {
                                Layout.fillWidth: true
                                text: root.title
                                ink: root.permission ? "c3" : root.critical ? "c2" : "c5"
                                elide: Text.ElideRight
                            }
                            H.HText {
                                text: Toasts.age(root.time, Toasts.now)
                                role: "caption"
                                ink: "c1"
                            }
                        }
                        H.HText {
                            visible: text.length > 0
                            Layout.fillWidth: true
                            text: root.bodyText
                            wrapMode: Text.Wrap
                            maximumLineCount: 4
                            elide: Text.ElideRight
                        }
                    }
                }

                H.Progress {
                    visible: root.value >= 0
                    Layout.fillWidth: true
                    value: Math.max(0, root.value)
                    count: Math.round(Math.max(0, root.value) * 100) + "%"
                }

                RowLayout {
                    visible: root.buttons.length > 0 || root.critical || root.permission
                    Layout.fillWidth: true
                    spacing: 8

                    Repeater {
                        model: root.buttons
                        H.InkButton {
                            required property var modelData
                            required property int index
                            text: modelData.text
                            ink: root.buttonInk(modelData, index)
                            onClicked: Notifications.attemptInvokeAction(root.nid, modelData.identifier)
                        }
                    }
                    H.InkButton {
                        visible: root.critical || root.permission
                        text: "dismiss"
                        ink: "c1"
                        onClicked: Notifications.discardNotification(root.nid)
                    }
                }
            }
        }
    }
}
