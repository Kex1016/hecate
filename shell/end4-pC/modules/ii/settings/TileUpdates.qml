pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// CakeOS updates: how far behind the installed system is, the newest commit
// subjects, and the two ways to apply them (both open the updater, which asks
// for the password).
DashTile {
    id: root

    readonly property var status: Updates.cakeosStatus
    readonly property var commits: (status?.commits ?? []).slice(0, 3)
    readonly property int behind: Updates.cakeos ? (status?.behind ?? 0) : Updates.count

    title: "updates"
    badge: root.behind > 0 ? String(root.behind) : ""

    function open(mode) {
        Updates.applyMode = mode;
        Updates.openUpdater();
    }

    Repeater {
        model: root.commits
        Row {
            id: commit
            required property var modelData
            spacing: H.HTokens.ch()
            H.HText { text: (commit.modelData.sha ?? "").slice(0, 7); role: "code"; ink: "gold" }
            H.HText {
                width: root.width - 2 * H.HTokens.space4 - x
                text: commit.modelData.subject ?? ""
                role: "code"
                elide: Text.ElideRight
            }
        }
    }
    H.HText {
        visible: root.commits.length === 0
        Layout.fillWidth: true
        text: !Updates.cakeos ? (Updates.count > 0 ? `${Updates.count} packages can be updated.` : "Everything is up to date.")
            : root.status?.error ? "The update check failed. Try again from the updater."
            : Updates.checking ? "Asking upstream…"
            : "CakeOS is up to date."
        role: "code"
        ink: "rose"
        wrapMode: Text.Wrap
    }

    Item { Layout.fillHeight: true }

    Row {
        visible: Updates.cakeos
        spacing: 8
        H.InkButton {
            visible: root.behind > 0
            text: "apply on boot"
            ink: "c3"
            onClicked: root.open("boot")
        }
        H.InkButton {
            visible: root.behind > 0
            text: "now"
            ink: "c1"
            onClicked: root.open("switch")
        }
        H.InkButton {
            visible: root.behind === 0
            text: "check"
            ink: "c1"
            onClicked: Updates.openUpdater()
        }
    }
}
