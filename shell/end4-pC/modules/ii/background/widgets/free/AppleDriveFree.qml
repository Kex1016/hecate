pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services.apple
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.background.widgets.system as System

// ls ~/iCloud, free · a pixel cloud beside the mount drawn as a tree. A folder that is still
// uploading carries three squares that step in foam; space used is twenty cells when the
// service reports it. The one thing to know stays under it: rclone can't reach a drive with
// Advanced Data Protection on. From services/apple/Drive.qml (rclone mount, started at login).
H.FreeWidget {
    id: root

    property int maxRows: 5

    readonly property string shownPath: Drive.mountPoint.replace(new RegExp(`^${Quickshell.env("HOME")}`), "~")
    // { used, total } in bytes, null until Drive knows
    readonly property var quota: Drive.quota
    readonly property var rows: Drive.mounted ? Drive.entries.slice(0, root.maxRows) : []
    readonly property bool anySyncing: root.rows.some(r => r.syncing > 0)
    property int step: 0

    label: `ls ${root.shownPath}`
    implicitWidth: 360

    // holds one of the service's `consumers` while the widget is live
    System.ConsumerTicket {
        service: Drive
        active: root.live
    }

    Timer {
        running: root.live && root.anySyncing
        interval: 500
        repeat: true
        onTriggered: root.step = (root.step + 1) % 3
    }

    function gb(bytes) {
        return Math.round(bytes / 1e9);
    }

    art: H.PixelGlyph {
        name: "cloud"
    }

    // the board's pre: one block of terminal text, no gap between its lines
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 0
        visible: Drive.mounted

        H.FreeText {
            visible: Drive.mounted
            role: "code"
            leading: H.HTokens.leadingPre
            ink: "c6"
            text: root.shownPath
        }

        Repeater {
            model: root.rows
            delegate: RowLayout {
                id: row
                required property var modelData
                required property int index
                Layout.fillWidth: true
                spacing: 8
                H.FreeText {
                    role: "code"
                    leading: H.HTokens.leadingPre
                    ink: "subtle"
                    text: row.index === root.rows.length - 1 && Drive.entries.length <= root.maxRows ? "└─" : "├─"
                }
                H.FreeText {
                    Layout.fillWidth: true
                    Layout.maximumWidth: implicitWidth
                    role: "code"
                    leading: H.HTokens.leadingPre
                    ink: row.modelData.dir ? "text" : "c1"
                    elide: Text.ElideRight
                    text: row.modelData.name
                }
                H.FreeText {
                    visible: row.modelData.syncing > 0
                    role: "code"
                    leading: H.HTokens.leadingPre
                    ink: "muted"
                    text: `${row.modelData.syncing} syncing`
                }
                H.PixelCells {
                    visible: row.modelData.syncing > 0
                    count: 3
                    lit: root.step
                    ink: "foam"
                    cell: 6
                    gap: 2
                }
                Item { Layout.fillWidth: true }
            }
        }
        H.FreeText {
            visible: Drive.mounted && Drive.entries.length > root.maxRows
            role: "code"
            leading: H.HTokens.leadingPre
            ink: "subtle"
            text: `└─ + ${Drive.entries.length - root.maxRows} more`
        }
    }

    // space used, twenty cells of 5%
    H.PixelCells {
        visible: Drive.mounted && root.quota !== null && (root.quota.total ?? 0) > 0
        Layout.topMargin: 4
        count: 20
        filled: root.quota && root.quota.total > 0 ? Math.round(root.quota.used / root.quota.total * 20) : 0
        ink: "foam"
    }
    H.FreeText {
        visible: Drive.mounted && root.quota !== null && (root.quota.total ?? 0) > 0
        role: "caption"
        ink: "muted"
        text: root.quota ? `${root.gb(root.quota.used)} of ${root.gb(root.quota.total)} GB` : ""
    }

    RowLayout {
        Layout.fillWidth: true
        visible: !Drive.mounted
        spacing: 8
        H.FreeText {
            Layout.fillWidth: true
            role: "code"
            leading: H.HTokens.leadingPre
            ink: Drive.status === "error" ? "love" : "muted"
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

    H.FreeText {
        Layout.fillWidth: true
        role: "caption"
        ink: "muted"
        wrapMode: Text.Wrap
        text: `rclone ${Drive.mounted ? "mounted at login" : "mount"} · ${Drive.adpNote}`
    }

    TapHandler {
        enabled: Drive.mounted
        onTapped: Drive.open()
    }
}
