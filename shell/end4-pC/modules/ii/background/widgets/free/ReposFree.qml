pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import qs.modules.ii.background.widgets.system as System

// git status ~/src · the free form of the repos widget: the checkouts as a tree, coloured by
// what each one needs (love changed, gold to push or pull, muted clean). Clicking a line
// opens a terminal there. The status is read by Repos in a child process, never on the UI
// thread.
H.FreeWidget {
    id: root

    property int maxRows: 8
    readonly property string home: Quickshell.env("HOME")
    readonly property string shownPath: Repos.path.startsWith(root.home + "/")
        ? `~${Repos.path.slice(root.home.length)}` : Repos.path
    readonly property var shown: Repos.repos.slice(0, root.maxRows)
    // one character of the code face, so the columns line up
    readonly property real cw: measure.implicitWidth
    readonly property int nameChars: shown.reduce((m, r) => Math.max(m, r.name.length), 4)
    readonly property int branchChars: shown.reduce((m, r) => Math.max(m, r.branch.length), 4)

    label: `git status ${root.shownPath}`
    implicitWidth: 360

    System.ConsumerTicket {
        service: Repos
        active: root.live
    }

    H.FreeText {
        id: measure
        visible: false
        role: "code"
        text: "M"
    }

    function status(r) {
        switch (r.state) {
        case "changed": return { ink: "love", text: `${r.changed} changed` };
        case "ahead": return { ink: "gold", text: `↑${r.ahead} unpushed` + (r.behind > 0 ? ` ↓${r.behind}` : "") };
        case "behind": return { ink: "gold", text: `↓${r.behind} behind` };
        default: return { ink: "subtle", text: "clean" };
        }
    }

    H.FreeText {
        visible: Repos.repos.length > 0
        role: "code"
        text: root.shownPath
    }

    Repeater {
        model: root.shown

        Item {
            id: row
            required property var modelData
            required property int index
            readonly property var st: root.status(modelData)
            readonly property bool last: index === root.shown.length - 1

            Layout.fillWidth: true
            implicitHeight: line.implicitHeight

            Rectangle {
                anchors.fill: parent
                anchors.leftMargin: -4
                anchors.rightMargin: -4
                color: H.HTokens.overlay
                visible: hover.hovered
            }

            Row {
                id: line
                spacing: root.cw * 2

                H.FreeText {
                    role: "code"
                    ink: "muted"
                    text: row.last ? "└─" : "├─"
                }
                H.FreeText {
                    width: root.nameChars * root.cw
                    role: "code"
                    ink: "text"
                    elide: Text.ElideRight
                    text: row.modelData.name
                }
                H.FreeText {
                    width: root.branchChars * root.cw
                    role: "code"
                    ink: "pine"
                    elide: Text.ElideRight
                    text: row.modelData.branch
                }
                H.FreeText {
                    role: "code"
                    ink: row.st.ink
                    text: row.st.text
                }
            }

            HoverHandler {
                id: hover
                cursorShape: Qt.PointingHandCursor
            }
            TapHandler {
                onTapped: Repos.openTerminal(row.modelData.path)
            }
        }
    }

    H.FreeText {
        visible: Repos.repos.length === 0
        role: "code"
        ink: "muted"
        text: Repos.loading ? "reading…" : `no checkouts in ${root.shownPath}`
    }

    H.FreeText {
        visible: Repos.repos.length > 0
        role: "caption"
        ink: "subtle"
        text: Repos.repos.length > root.maxRows
            ? `${Repos.repos.length - root.maxRows} more · click a line: a terminal there`
            : "click a line: a terminal there"
    }
}
