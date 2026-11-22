pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H
import ".." as W

// update-cakeos --explain, free · the pending CakeOS commits in plain words, drawn like the
// updates graph: a gold `*` and gold area tag per change, a muted `│` under the tag down its
// wrapped lines, then the gold `update · N` button. `explain` sends the commit subjects (print
// them with `sent`); the update is its own button and is never pressed for you.
H.FreeWidget {
    id: root

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

    label: "update-cakeos --explain"
    implicitWidth: 380
    spacing: 4

    W.ClaudeRunner {
        id: runner
        system: "You explain software updates to the person who owns the machine. Given git commit subjects of a NixOS config, write at most four lines, one per change that matters to them, in the form `word: one plain sentence`. The word is a short topic (greetd, flake, theme...). No markdown, no jargon you do not explain, nothing the commits do not say."
        input: root.sentText
    }

    // the board's pre: one block of terminal text, no gap between its lines
    ColumnLayout {
        Layout.fillWidth: true
        spacing: 0
        visible: runner.done && runner.error === ""

        Repeater {
            model: runner.done && runner.error === "" ? root.notes : []

            Row {
                id: n
                required property var modelData
                Layout.fillWidth: true
                spacing: 8

                H.FreeText { role: "code"; leading: H.HTokens.leadingPre; ink: "gold"; text: "*" }
                // six characters, as the board pads its tags; a longer tag pushes its line over
                Item {
                    width: Math.max(6 * H.HTokens.ch(tag.font.pixelSize), tag.implicitWidth)
                    height: body.implicitHeight
                    H.FreeText {
                        id: tag
                        role: "code"
                        leading: H.HTokens.leadingPre
                        ink: "gold"
                        text: n.modelData.tag
                    }
                    // the graph's line runs down the wrapped lines of this change, under its tag
                    H.FreeText {
                        role: "code"
                        leading: H.HTokens.leadingPre
                        ink: "muted"
                        text: "\n" + Array.from({ length: Math.max(0, body.lineCount - 1) }, () => "│").join("\n")
                    }
                }
                H.FreeText {
                    id: body
                    width: root.width - x
                    role: "code"
                    leading: H.HTokens.leadingPre
                    wrapMode: Text.WordWrap
                    text: n.modelData.text
                }
            }
        }
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: !runner.done && !runner.running
        role: "code"
        leading: H.HTokens.leadingPre
        ink: "subtle"
        wrapMode: Text.WordWrap
        text: !Updates.cakeos ? "no cakeos updater on this system" : root.behind ? `${root.commits.length} commit${root.commits.length === 1 ? "" : "s"} waiting. Nothing explained yet.` : "cakeos is up to date"
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: runner.running || runner.error !== ""
        role: "caption"
        ink: runner.error !== "" ? "love" : "muted"
        text: runner.running ? "reading the commits…" : runner.error
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.commits.length > 0
        lines: root.commits.map(c => "# " + (c.sha ?? "").slice(0, 7) + " " + (c.subject ?? ""))
    }

    Row {
        Layout.topMargin: 6
        visible: root.behind
        spacing: 8
        H.InkButton {
            visible: runner.done && runner.error === ""
            ink: "c3"
            text: Updates.applying ? "updating…" : (Updates.count > 0 ? `update · ${Updates.count}` : `update · ${root.commits.length}`)
            onClicked: Updates.openUpdater()
        }
        H.InkButton {
            ink: runner.done && runner.error === "" ? "c1" : "c3"
            flat: runner.done && runner.error === ""
            text: runner.running ? "reading…" : runner.done ? "explain again" : "explain"
            enabled: !runner.running
            onClicked: runner.run()
        }
        H.InkButton {
            flat: true
            role: "caption"
            text: "sent"
            on: root.showContext
            onClicked: root.showContext = !root.showContext
        }
    }

    H.FreeText {
        visible: runner.done && runner.error === ""
        role: "caption"
        ink: "muted"
        text: `from ${root.commits.length} commit${root.commits.length === 1 ? "" : "s"} and the flake.lock diff`
    }
}
