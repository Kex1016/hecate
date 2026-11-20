pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H

// notify: nushell · a toast for a long command that failed while you were
// elsewhere. The shell side (a nushell hook, not part of this widget) writes
// failed-command.json: { "command", "status", "durationMs", "cwd", "output" }
// where output is the last lines it saw, if any. The widget only shows it.
// `ask claude` sends exactly that record, listed with `sent`; nothing is sent
// before the press, and `dismiss` clears the record.
H.HWindow {
    id: root

    property bool live: true
    property bool showContext: false
    property var rec: null

    readonly property bool present: root.rec !== null && (root.rec.command ?? "") !== ""
    readonly property string home: Quickshell.env("HOME")
    readonly property string cwd: String(root.rec?.cwd ?? "").replace(root.home, "~")
    readonly property string sentText: !root.present ? "" : [
        `command: ${root.rec.command}`,
        `exit status: ${root.rec.status ?? "?"}`,
        `directory: ${root.cwd}`
    ].concat(root.rec.output ? ["last output:", String(root.rec.output).slice(-1500)] : []).join("\n")

    function took(ms) {
        const s = Math.round((Number(ms) || 0) / 1000);
        return s >= 60 ? `${Math.floor(s / 60)}m ${s % 60}s` : `${s}s`;
    }

    variant: "love"
    label: "notify: nushell"
    widget: true
    implicitWidth: 420
    note: runner.running ? "asking…" : runner.error !== "" ? runner.error : root.present && !runner.done ? "ask claude sends the command and its last output" : ""

    ClaudeRunner {
        id: runner
        system: "A shell command failed. Explain in at most three plain sentences the most likely cause and the one thing to try next. No markdown. If you cannot tell from what is given, say so."
        input: root.sentText
    }

    FileView {
        id: file
        path: FileUtils.trimFileProtocol(`${Directories.state}/user/failed-command.json`)
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const o = JSON.parse(file.text());
                runner.reset();
                root.rec = o && o.command ? o : null;
            } catch (e) {
                root.rec = null;
            }
        }
        onLoadFailed: root.rec = null
    }

    H.HText {
        Layout.fillWidth: true
        role: "code"
        wrapMode: Text.WordWrap
        textFormat: Text.StyledText
        text: root.present
            ? `<font color="${H.HTokens.gold}">${root.rec.command.replace(/&/g, "&amp;").replace(/</g, "&lt;")}</font> failed after ${root.took(root.rec.durationMs)} in <font color="${H.HTokens.foam}">${root.cwd}</font>`
            : "no failed command"
    }

    H.HText {
        Layout.fillWidth: true
        visible: runner.done && runner.error === "" && runner.result !== ""
        role: "code"
        ink: "c4"
        wrapMode: Text.WordWrap
        text: runner.result
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.present
        lines: root.sentText.split("\n").slice(0, 14).map(l => "# " + l.slice(0, 80))
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        visible: root.present
        H.InkButton {
            visible: !runner.done || runner.error !== ""
            ink: "c5"
            text: runner.running ? "asking…" : "ask claude"
            enabled: !runner.running
            onClicked: runner.run()
        }
        H.InkButton {
            ink: "c1"
            text: "dismiss"
            onClicked: {
                runner.reset();
                root.rec = null;
                file.setText("");
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
