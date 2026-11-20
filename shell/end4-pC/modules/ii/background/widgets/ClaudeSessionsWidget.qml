pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// claude: sessions · the Claude Code sessions that are running, one line each:
// project on the left, what it is doing on the right. A click opens the
// project's directory in a terminal. Reads ~/.claude/sessions through the
// ClaudeUsage service; nothing is sent anywhere.
H.HWindow {
    id: root

    property bool live: true
    property int maxRows: 5

    label: "claude: sessions"
    widget: true
    implicitWidth: 400

    function project(s) {
        const cwd = String(s.cwd ?? "").replace(/\/+$/, "");
        const base = cwd.split("/").pop();
        return (base || s.name || "session").toLowerCase();
    }

    function stateOf(s) {
        const st = String(s.status ?? "").toLowerCase();
        if (/wait|input|permission|allow/.test(st)) return { text: "waiting · allow?", ink: "c3" };
        if (/busy|work|run|active/.test(st)) return { text: "working", ink: "c5" };
        if (/idle|done|stop|end/.test(st)) return { text: "done", ink: "c1" };
        return { text: st || "open", ink: "c1" };
    }

    Repeater {
        model: ClaudeUsage.sessions.slice(0, root.maxRows)

        MouseArea {
            id: row
            required property var modelData
            readonly property var st: root.stateOf(modelData)
            Layout.fillWidth: true
            implicitHeight: line.implicitHeight
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: Quickshell.execDetached({
                command: ["sh", "-c", Config.options.apps.terminal],
                workingDirectory: modelData.cwd || Quickshell.env("HOME"),
            })

            RowLayout {
                id: line
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
                H.HText {
                    Layout.fillWidth: true
                    role: "code"
                    ink: row.containsMouse ? "text" : "c6"
                    elide: Text.ElideRight
                    text: root.project(row.modelData)
                }
                H.HText {
                    role: "code"
                    ink: row.st.ink
                    text: row.st.text
                }
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: ClaudeUsage.sessions.length === 0
        role: "code"
        ink: "c1"
        text: "no claude code sessions are open"
    }

    H.HText {
        Layout.fillWidth: true
        visible: ClaudeUsage.sessions.length > root.maxRows
        role: "caption"
        ink: "c1"
        text: `and ${ClaudeUsage.sessions.length - root.maxRows} more`
    }
}
