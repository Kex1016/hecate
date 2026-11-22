pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.services.claude
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

// claude ps, free · one line per Claude Code session behind a state square: gold waits for
// you, pine works (the word in text), love failed, nothing lit is done. Under the list, the
// first permission request that is waiting gets allow and deny, with the request itself
// printed beside them. Nothing is allowed for you: that is the bridge's rule (ClaudeSessions).
// Without the bridge the lines come from the usage logs and there are no buttons.
H.FreeWidget {
    id: root

    property int maxRows: 5

    // one shape from either source: { name, state, extra, id, cwd }
    readonly property var rows: {
        if (ClaudeSessions.available) {
            return ClaudeSessions.sessions.filter(s => !s.ended || s.state !== "idle").map(s => {
                const failed = (s.error ?? "") !== "";
                const st = failed ? "failed" : s.state === "waiting" ? "waiting" : s.state === "working" ? "working" : "done";
                let extra = "";
                if (st === "waiting") extra = s.waitingFor === "permission" ? "allow " + (root.requestOf(s)?.verb ?? "") + "?" : "answer it";
                else if (st === "working") extra = s.turnStartedAt > 0 ? root.age(s.turnStartedAt) : "";
                else if (st === "failed") extra = String(s.error).replace(/_/g, " ");
                return { id: s.id, cwd: s.cwd, name: String(s.name || "session").toLowerCase(), state: st, extra: extra.trim() };
            });
        }
        return ClaudeUsage.sessions.map(s => {
            const t = String(s.status ?? "").toLowerCase();
            const st = /wait|input|permission|allow/.test(t) ? "waiting" : /busy|work|run|active/.test(t) ? "working" : "done";
            const base = String(s.cwd ?? "").replace(/\/+$/, "").split("/").pop();
            return { id: s.id ?? "", cwd: s.cwd, name: (base || s.name || "session").toLowerCase(), state: st, extra: st === "waiting" ? "allow?" : "" };
        });
    }
    readonly property var firstWaiting: rows.find(r => r.state === "waiting") ?? null
    readonly property var request: ClaudeSessions.available && firstWaiting ? ClaudeSessions.requestsFor(firstWaiting.id)[0] ?? null : null

    label: "claude ps"
    implicitWidth: 360
    spacing: 4

    function requestOf(s) {
        return ClaudeSessions.requestsFor(s.id)[0] ?? null;
    }

    function age(ms) {
        const m = Math.max(0, Math.floor((Date.now() - ms) / 60000));
        return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
    }

    function inkOf(state) {
        return state === "waiting" ? "gold" : state === "working" ? "pine" : state === "failed" ? "love" : "muted";
    }

    Repeater {
        model: root.rows.slice(0, root.maxRows)

        MouseArea {
            id: row
            required property var modelData
            Layout.fillWidth: true
            implicitHeight: line.implicitHeight
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            // a click opens the project's directory in a terminal, as in the window
            onClicked: Quickshell.execDetached({
                command: ["sh", "-c", Config.options.apps.terminal],
                workingDirectory: modelData.cwd || Quickshell.env("HOME"),
            })

            Row {
                id: line
                spacing: 8
                H.PixelCells {
                    anchors.verticalCenter: parent.verticalCenter
                    count: 1
                    filled: row.modelData.state === "done" ? 0 : 1
                    ink: root.inkOf(row.modelData.state)
                }
                H.FreeText {
                    width: Math.max(92, implicitWidth)
                    role: "code"
                    ink: row.containsMouse ? "text" : "foam"
                    text: row.modelData.name
                }
                Row {
                    spacing: 0
                    H.FreeText {
                        role: "code"
                        ink: row.modelData.state === "waiting" ? "gold" : row.modelData.state === "failed" ? "love" : row.modelData.state === "working" ? "text" : "subtle"
                        text: row.modelData.state
                    }
                    H.FreeText {
                        visible: row.modelData.extra !== ""
                        role: "code"
                        ink: row.modelData.state === "waiting" ? "gold" : row.modelData.state === "failed" ? "love" : "subtle"
                        text: (row.modelData.state === "working" ? " " : " · ") + row.modelData.extra
                    }
                }
            }
        }
    }

    H.FreeText {
        visible: root.rows.length === 0
        role: "code"
        ink: "subtle"
        text: "no claude code sessions are open"
    }

    H.FreeText {
        visible: root.rows.length > root.maxRows
        role: "caption"
        ink: "muted"
        text: `and ${root.rows.length - root.maxRows} more`
    }

    // the request it would answer, printed next to the buttons
    Row {
        Layout.topMargin: 8
        visible: root.request !== null
        spacing: 8
        H.InkButton {
            ink: "c3"
            text: "allow"
            onClicked: ClaudeSessions.allow(root.request.id)
        }
        H.InkButton {
            ink: "c1"
            text: "deny"
            onClicked: ClaudeSessions.deny(root.request.id)
        }
        H.FreeText {
            anchors.verticalCenter: parent.verticalCenter
            role: "caption"
            ink: "muted"
            text: root.request ? `${root.request.name} · ${root.request.verb} ${root.request.subject}` : ""
        }
    }
}
