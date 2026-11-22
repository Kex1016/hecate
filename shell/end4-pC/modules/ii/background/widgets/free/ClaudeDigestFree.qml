pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.claude
import qs.modules.common.widgets.haiiro as H
import ".." as W

// notify: digest, free · what came in while the screen was locked: a gold pixel bell and the
// count in Jacquard, the ones that matter on gold » lines, one button for the rest. Counting is
// local. `pick what matters` sends app, title and the first 160 characters of each (print them
// with `sent`) to Claude; `clear the other N` is a separate press.
H.FreeWidget {
    id: root

    property bool showContext: false
    property var cleared: []

    readonly property var missed: Notifications.list.filter(n => n.time >= ClaudeBrief.lockedAt && ClaudeBrief.lockedAt > 0 && !root.cleared.includes(n.notificationId)).slice(0, 40)
    readonly property string sentText: root.missed.map((n, i) => `${i + 1}|${n.appName}|${n.summary}|${String(n.body).replace(/\s+/g, " ").slice(0, 160)}`).join("\n")

    // "3: reason" lines -> indices into `missed`
    readonly property var picks: runner.result.split("\n")
        .map(l => l.match(/^\s*(\d+)\s*[:.\-]\s*(.*)$/))
        .filter(m => m && Number(m[1]) >= 1 && Number(m[1]) <= root.missed.length)
        .map(m => ({ n: root.missed[Number(m[1]) - 1], why: m[2] }))
    readonly property var rest: root.missed.filter(n => !root.picks.some(p => p.n === n))
    readonly property bool picked: runner.done && runner.error === ""

    label: "notify: digest"
    implicitWidth: 400
    spacing: 10

    W.ClaudeRunner {
        id: runner
        system: "You triage desktop notifications for one person. Each input line is `number|app|title|start of text`. Reply with only the lines that need a human soon (a person asking something, a failure, a deadline), as `number: five words on why`. Nothing else. If none matter, reply with exactly `none`."
        input: root.sentText
    }

    Row {
        spacing: 16
        H.PixelGlyph {
            anchors.verticalCenter: parent.verticalCenter
            name: "bell"
            cell: 5
        }
        H.FreeText {
            anchors.verticalCenter: parent.verticalCenter
            role: "display"
            size: 72
            text: String(root.missed.length)
        }
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 0
            H.FreeText {
                role: "caption"
                ink: "subtle"
                text: root.missed.length === 0 ? "nothing came in while you were away" : "while you were away"
            }
            H.FreeText {
                visible: root.picked && root.missed.length > 0
                role: "caption"
                ink: "gold"
                text: ` · ${root.picks.length} matter${root.picks.length === 1 ? "s" : ""}`
            }
        }
    }

    Repeater {
        model: root.picked ? root.picks : []

        Row {
            id: p
            required property var modelData
            spacing: 8
            H.FreeText { role: "body"; ink: "gold"; text: "»" }
            H.FreeText {
                width: root.width - x
                role: "body"
                elide: Text.ElideRight
                text: `${p.modelData.n.summary || p.modelData.n.appName}${p.modelData.why ? " · " + p.modelData.why : ""}`
            }
        }
    }

    H.FreeText {
        visible: runner.running || runner.error !== "" || (root.missed.length > 0 && !runner.done)
        role: "caption"
        ink: runner.error !== "" ? "love" : "muted"
        text: runner.running ? "reading them…" : runner.error !== "" ? runner.error : "pick what matters sends app, title and start of each"
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.missed.length > 0
        lines: root.sentText.split("\n").filter(l => l.length > 0).map(l => "# " + l)
    }

    Row {
        visible: root.missed.length > 0
        spacing: 8
        H.InkButton {
            visible: !root.picked
            ink: "c3"
            text: runner.running ? "reading…" : "pick what matters"
            enabled: !runner.running
            onClicked: runner.run()
        }
        H.InkButton {
            visible: root.picked && root.rest.length > 0
            ink: "c1"
            text: `clear the other ${root.rest.length}`
            onClicked: {
                const ids = root.rest.map(n => n.notificationId);
                root.cleared = root.cleared.concat(ids);
                Notifications.discardNotifications(ids);
            }
        }
        H.InkButton {
            flat: true
            role: "caption"
            text: "sent"
            on: root.showContext
            onClicked: root.showContext = !root.showContext
        }
    }
}
