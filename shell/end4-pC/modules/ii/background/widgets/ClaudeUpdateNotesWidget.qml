pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// update-cakeos: what changed · the pending CakeOS commits explained in plain
// words before you apply them. Press `explain` and the commit subjects (listed
// below with `sent`) go to Claude; the update itself is a separate button and
// is never pressed for you.
H.HWindow {
    id: root

    property bool live: true
    property bool showContext: false

    readonly property var commits: (Updates.cakeosStatus?.commits ?? []).slice(0, 30)
    readonly property bool behind: Updates.cakeos && root.commits.length > 0
    readonly property string sentText: root.commits.map(c => `${(c.sha ?? "").slice(0, 7)} ${c.subject ?? ""}`).join("\n")

    // "tag: sentence" per line
    readonly property var notes: runner.result.split("\n")
        .map(l => l.replace(/^[-*\s]+/, "").trim())
        .filter(l => l.length > 0)
        .map(l => {
            const i = l.indexOf(":");
            return i > 0 && i < 16 ? { tag: l.slice(0, i).toLowerCase(), text: l.slice(i + 1).trim() } : { tag: "", text: l };
        })

    label: "update-cakeos: what changed"
    widget: true
    implicitWidth: 420
    note: {
        if (runner.running) return "reading the commits…";
        if (runner.error !== "") return runner.error;
        if (runner.done) return `from ${root.commits.length} commit${root.commits.length === 1 ? "" : "s"}`;
        return root.behind ? `${root.commits.length} waiting · explain sends their subjects` : "";
    }

    ClaudeRunner {
        id: runner
        system: "You explain software updates to the person who owns the machine. Given git commit subjects of a NixOS config, write at most four lines, one per change that matters to them, in the form `word: one plain sentence`. The word is a short topic (greetd, flake, theme...). No markdown, no jargon you do not explain, nothing the commits do not say."
        input: root.sentText
    }

    Repeater {
        model: runner.done && runner.error === "" ? root.notes : []

        RowLayout {
            id: n
            required property var modelData
            Layout.fillWidth: true
            spacing: H.HTokens.ch(H.HTokens.fontHeadingSm)
            H.HText {
                Layout.alignment: Qt.AlignTop
                role: "code"
                ink: "c3"
                visible: n.modelData.tag !== ""
                text: n.modelData.tag
            }
            H.HText {
                Layout.fillWidth: true
                role: "code"
                wrapMode: Text.WordWrap
                text: n.modelData.text
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: !runner.done && !runner.running
        role: "code"
        ink: "c1"
        wrapMode: Text.WordWrap
        text: !Updates.cakeos ? "no cakeos updater on this system" : root.behind ? "Nothing explained yet." : "cakeos is up to date"
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.commits.length > 0
        lines: root.commits.map(c => "# " + (c.sha ?? "").slice(0, 7) + " " + (c.subject ?? ""))
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        visible: root.behind
        H.InkButton {
            ink: runner.done && runner.error === "" ? "c1" : "c3"
            text: runner.running ? "reading…" : runner.done ? "explain again" : "explain"
            enabled: !runner.running
            onClicked: runner.run()
        }
        H.InkButton {
            visible: runner.done && runner.error === ""
            ink: "c3"
            text: Updates.applying ? "updating…" : "update"
            onClicked: Updates.openUpdater()
        }
        H.Chip {
            text: "sent"
            on: root.showContext
            interactive: true
            onClicked: root.showContext = !root.showContext
        }
    }
}
