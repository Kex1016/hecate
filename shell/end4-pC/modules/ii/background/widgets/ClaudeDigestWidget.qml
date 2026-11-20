pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.claude
import qs.modules.common.widgets.haiiro as H

// notify: digest · after you unlock, the notifications that came in while the
// screen was locked. Counting them is local. `pick what matters` sends the
// app, title and the first 160 characters of each (see `sent`) to Claude, which
// answers with the ones worth reading; `clear the rest` is a separate press.
H.HWindow {
    id: root

    property bool live: true
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

    label: "notify: digest"
    widget: true
    implicitWidth: 420
    note: runner.running ? "reading them…" : runner.error !== "" ? runner.error : root.missed.length > 0 && !runner.done ? "pick what matters sends app, title and start of each" : ""

    ClaudeRunner {
        id: runner
        system: "You triage desktop notifications for one person. Each input line is `number|app|title|start of text`. Reply with only the lines that need a human soon (a person asking something, a failure, a deadline), as `number: five words on why`. Nothing else. If none matter, reply with exactly `none`."
        input: root.sentText
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
        H.HText {
            role: "code"
            text: root.missed.length === 0 ? "Nothing came in while you were away." : `${root.missed.length} while you were away.`
        }
        H.HText {
            visible: runner.done && runner.error === "" && root.missed.length > 0
            role: "code"
            ink: "c3"
            text: `${root.picks.length} matter${root.picks.length === 1 ? "s" : ""}:`
        }
    }

    Repeater {
        model: runner.done && runner.error === "" ? root.picks : []

        RowLayout {
            id: p
            required property var modelData
            Layout.fillWidth: true
            spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
            H.HText { role: "code"; ink: "c4"; text: "»" }
            H.HText {
                Layout.fillWidth: true
                role: "code"
                elide: Text.ElideRight
                text: `${p.modelData.n.summary || p.modelData.n.appName}${p.modelData.why ? " · " + p.modelData.why : ""}`
            }
        }
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.missed.length > 0
        lines: root.sentText.split("\n").filter(l => l.length > 0).map(l => "# " + l)
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        visible: root.missed.length > 0
        H.InkButton {
            ink: runner.done && runner.error === "" ? "c1" : "c3"
            visible: !runner.done || runner.error !== ""
            text: runner.running ? "reading…" : "pick what matters"
            enabled: !runner.running
            onClicked: runner.run()
        }
        H.InkButton {
            visible: runner.done && runner.error === "" && root.rest.length > 0
            ink: "c1"
            text: `clear the other ${root.rest.length}`
            onClicked: {
                const ids = root.rest.map(n => n.notificationId);
                root.cleared = root.cleared.concat(ids);
                Notifications.discardNotifications(ids);
            }
        }
        H.Chip {
            text: "sent"
            on: root.showContext
            interactive: true
            onClicked: root.showContext = !root.showContext
        }
    }
}
