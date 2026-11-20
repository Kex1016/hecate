pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// updates · the CakeOS commits that are not installed yet, newest first, and the button
// that opens the updater. Elsewhere (Arch) it is the package count.
H.HWindow {
    id: root

    property bool live: true
    property int maxRows: 3
    readonly property var status: Updates.cakeosStatus
    readonly property var commits: root.status?.commits ?? []
    readonly property bool known: Updates.cakeos && !!root.status && !root.status.error
    // an unknown installed version counts as behind: updating records it
    readonly property bool unknownVersion: root.known && (root.status.behind ?? null) === null
    readonly property bool behind: root.known && (root.unknownVersion || root.status.behind > 0)

    label: "git log"
    widget: true
    implicitWidth: 360

    // "fix(greetd): keep gpu on handover" -> "keep gpu on handover"
    function subject(s) {
        return (s ?? "").replace(/^[a-z]+(\([^)]*\))?!?:\s*/i, "").split("\n")[0];
    }

    Repeater {
        model: Updates.cakeos ? root.commits.slice(0, root.maxRows) : []

        RowLayout {
            id: commit
            required property var modelData
            Layout.fillWidth: true
            spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)

            H.HText {
                role: "code"
                ink: "c3"
                text: (commit.modelData.sha ?? "").slice(0, 7)
            }
            H.HText {
                Layout.fillWidth: true
                role: "code"
                elide: Text.ElideRight
                text: root.subject(commit.modelData.subject)
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: text.length > 0
        role: "code"
        ink: Updates.cakeos && root.status?.error ? "c2" : "c1"
        elide: Text.ElideRight
        text: {
            if (!Updates.available) return "no updater on this system";
            if (!Updates.cakeos) return Updates.count > 0 ? `${Updates.count} packages to update` : "up to date";
            if (root.status === null) return Updates.checking ? "checking…" : "not checked yet";
            if (root.status.error) return String(root.status.error).toLowerCase();
            if (root.unknownVersion) return "installed version unknown";
            if (root.status.behind === 0) return "cakeos is up to date";
            if (root.commits.length > root.maxRows) return `and ${root.commits.length - root.maxRows} more`;
            return "";
        }
    }

    H.InkButton {
        visible: root.behind
        ink: "c3"
        text: Updates.applying ? "updating…" : "update"
        onClicked: Updates.openUpdater()
    }
}
