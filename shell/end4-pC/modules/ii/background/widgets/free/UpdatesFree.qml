pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// git log --graph · the free form of the updates widget: the CakeOS commits that are not
// installed yet as a git graph, down to the installed one in pine, and the gold update button
// that opens the updater. Elsewhere (Arch) it is the package count.
H.FreeWidget {
    id: root

    property int maxRows: 3
    readonly property var status: Updates.cakeosStatus
    readonly property var commits: root.status?.commits ?? []
    readonly property bool known: Updates.cakeos && !!root.status && !root.status.error
    // an unknown installed version counts as behind: updating records it
    readonly property bool unknownVersion: root.known && (root.status.behind ?? null) === null
    readonly property bool behind: root.known && (root.unknownVersion || root.status.behind > 0)
    readonly property string installed: String(root.status?.installed ?? "").slice(0, 7)

    label: "git log --graph"
    implicitWidth: 360

    // "fix(greetd): keep gpu on handover" -> "keep gpu on handover"
    function subject(s) {
        return (s ?? "").replace(/^[a-z]+(\([^)]*\))?!?:\s*/i, "").split("\n")[0];
    }

    Repeater {
        model: Updates.cakeos ? root.commits.slice(0, root.maxRows) : []

        Row {
            id: commit
            required property var modelData
            Layout.fillWidth: true
            spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)

            H.FreeText { role: "code"; ink: "gold"; text: "*" }
            H.FreeText { role: "code"; ink: "gold"; text: (commit.modelData.sha ?? "").slice(0, 7) }
            H.FreeText {
                width: Math.max(0, root.width - x)
                role: "code"
                elide: Text.ElideRight
                text: root.subject(commit.modelData.subject)
            }
        }
    }

    // the graph's line, down to the installed commit
    Row {
        visible: root.behind
        spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
        H.FreeText { role: "code"; ink: "muted"; text: "¦" }
        H.FreeText {
            visible: root.commits.length > root.maxRows
            role: "code"
            ink: "muted"
            text: `and ${root.commits.length - root.maxRows} more`
        }
    }

    Row {
        visible: root.known
        spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
        H.FreeText { role: "code"; ink: "pine"; text: "*" }
        H.FreeText { role: "code"; ink: "pine"; text: root.installed.length > 0 ? root.installed : "unknown" }
        H.FreeText { role: "code"; ink: "subtle"; text: "installed" }
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: text.length > 0
        role: "code"
        ink: Updates.cakeos && root.status?.error ? "love" : "muted"
        elide: Text.ElideRight
        text: {
            if (!Updates.available) return "no updater on this system";
            if (!Updates.cakeos) return Updates.count > 0 ? `${Updates.count} packages to update` : "up to date";
            if (root.status === null) return Updates.checking ? "checking…" : "not checked yet";
            if (root.status.error) return String(root.status.error).toLowerCase();
            if (root.status.behind === 0) return "cakeos is up to date";
            return "";
        }
    }

    H.InkButton {
        Layout.topMargin: 6
        visible: root.behind
        ink: "c3"
        text: Updates.applying ? "updating…" : (Updates.count > 0 ? `update · ${Updates.count}` : "update")
        onClicked: Updates.openUpdater()
    }
}
