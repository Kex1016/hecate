pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H

// repos · git status of the checkouts under ~/src, the ones that need you first:
// uncommitted work in love, unpushed or behind in gold, clean muted. Clicking a row opens
// a terminal there. The status is read by Repos in a child process, never on the UI thread.
H.HWindow {
    id: root

    property bool live: true
    property int maxRows: 8
    readonly property string home: Quickshell.env("HOME")
    readonly property string shownPath: Repos.path.startsWith(root.home + "/")
        ? `~${Repos.path.slice(root.home.length)}` : Repos.path

    label: `git status: ${root.shownPath}`
    widget: true
    implicitWidth: 360

    ConsumerTicket {
        service: Repos
        active: root.live
    }

    function status(r) {
        switch (r.state) {
        case "changed": return { ink: "c2", text: `${r.changed} changed` };
        case "ahead": return { ink: "c3", text: `↑${r.ahead} unpushed` + (r.behind > 0 ? ` ↓${r.behind}` : "") };
        case "behind": return { ink: "c3", text: `↓${r.behind} behind` };
        default: return { ink: "c1", text: "clean" };
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.space1

        Repeater {
            model: Repos.repos.slice(0, root.maxRows)

            Item {
                id: row
                required property var modelData
                readonly property var st: root.status(modelData)

                Layout.fillWidth: true
                implicitHeight: line.implicitHeight

                Rectangle {
                    anchors.fill: parent
                    anchors.leftMargin: -H.HTokens.ch(H.HTokens.fontHeadingSm)
                    anchors.rightMargin: -H.HTokens.ch(H.HTokens.fontHeadingSm)
                    color: H.HTokens.overlay
                    visible: hover.hovered
                }

                RowLayout {
                    id: line
                    width: parent.width
                    spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)

                    H.HText {
                        Layout.maximumWidth: line.width * 0.5
                        role: "code"
                        ink: "c6"
                        elide: Text.ElideRight
                        text: row.modelData.name
                    }
                    H.HText {
                        Layout.fillWidth: true
                        role: "code"
                        ink: "c5"
                        elide: Text.ElideRight
                        text: row.modelData.branch
                    }
                    H.HText {
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
    }

    H.HText {
        visible: Repos.repos.length === 0
        role: "code"
        ink: "c1"
        text: Repos.loading ? "reading…" : `no checkouts in ${root.shownPath}`
    }

    H.HText {
        visible: Repos.repos.length > 0
        role: "caption"
        ink: "c1"
        text: Repos.repos.length > root.maxRows
            ? `${Repos.repos.length - root.maxRows} more · click: open a terminal there`
            : "click: open a terminal there"
    }
}
