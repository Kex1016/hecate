pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// ls: ~/iCloud · the top of iCloud Drive as the file manager shows it, what is still
// uploading, and the one thing to know: rclone can't reach a drive with Advanced Data
// Protection on. From services/apple/Drive.qml (rclone mount, started at login).
H.HWindow {
    id: root

    property bool live: true
    property int maxRows: 5

    readonly property string shownPath: Drive.mountPoint.replace(new RegExp(`^${Quickshell.env("HOME")}`), "~")

    label: `ls: ${root.shownPath}`
    widget: true
    implicitWidth: 360

    property bool held: false
    function hold() {
        if (root.live && !root.held) { Drive.consumers++; root.held = true; }
        else if (!root.live && root.held) { Drive.consumers--; root.held = false; }
    }
    onLiveChanged: hold()
    Component.onCompleted: hold()
    Component.onDestruction: if (root.held) Drive.consumers--

    Repeater {
        model: Drive.mounted ? Drive.entries.slice(0, root.maxRows) : []
        delegate: RowLayout {
            id: row
            required property var modelData
            Layout.fillWidth: true
            spacing: H.HTokens.ch(14)
            H.HText { role: "code"; ink: row.modelData.dir ? "c6" : "c1"; text: "»" }
            H.HText {
                Layout.fillWidth: true
                role: "code"
                elide: Text.ElideRight
                textFormat: Text.StyledText
                text: row.modelData.name.replace(/&/g, "&amp;").replace(/</g, "&lt;")
                    + (row.modelData.syncing > 0 ? ` <font color="${H.HTokens.muted}">· ${row.modelData.syncing} syncing</font>` : "")
            }
        }
    }
    H.HText {
        visible: Drive.mounted && Drive.entries.length > root.maxRows
        role: "code"
        ink: "c1"
        text: `» + ${Drive.entries.length - root.maxRows} more`
    }

    RowLayout {
        Layout.fillWidth: true
        visible: !Drive.mounted
        spacing: 8
        H.HText {
            Layout.fillWidth: true
            role: "code"
            ink: Drive.status === "error" ? "c2" : "c1"
            wrapMode: Text.Wrap
            text: Drive.status === "missing" ? "rclone is not installed"
                : Drive.status === "mounting" ? "mounting…"
                : Drive.status === "signIn" ? "sign in to iCloud again"
                : Drive.status === "error" ? (Drive.message || "the mount stopped")
                : !Drive.configured ? (Drive.appleId ? `not set up for ${Drive.appleId}` : "not set up")
                : "not mounted"
        }
        H.InkButton {
            visible: Drive.installed && (!Drive.configured || Drive.status === "signIn")
            ink: "c3"
            role: "code"
            text: "sign in"
            onClicked: Drive.signIn()
        }
        H.InkButton {
            visible: Drive.installed && Drive.configured && Drive.status !== "signIn" && Drive.status !== "mounting"
            ink: "c1"
            role: "code"
            text: "mount"
            onClicked: Drive.mount()
        }
    }

    H.HText {
        Layout.fillWidth: true
        role: "caption"
        ink: "c1"
        wrapMode: Text.Wrap
        text: `rclone mount · ${Drive.adpNote}`
    }

    TapHandler {
        enabled: Drive.mounted
        onTapped: Drive.open()
    }
}
