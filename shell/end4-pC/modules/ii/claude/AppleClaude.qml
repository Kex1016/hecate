pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import "AppleLogic.js" as Logic

// Runs one question through the local Claude Code login: `claude -p` with
// every tool switched off, so it can only answer in text. It never runs on
// its own; a card calls run() after a press and shows what it sent first.
//
//   AppleClaude { id: claude }
//   claude.run(prompt)  ->  finished(ok, text, error)
//
// HECATE_CLAUDE names another executable (a fake one for tests). It goes
// through `sh -c` so a missing binary still ends in an exit status:
// a QML Process whose program is absent never reports back.
Item {
    id: root

    readonly property string bin: Quickshell.env("HECATE_CLAUDE") || "claude"
    property bool busy: false
    property string error: ""
    property string reply: ""
    // The exact text of the last prompt, for the "sent" line.
    property string sent: ""
    property int timeoutMs: 120000

    signal finished(bool ok, string text, string error)

    function run(prompt) {
        if (root.busy)
            return false;
        root.busy = true;
        root.error = "";
        root.reply = "";
        root.sent = prompt;
        proc.command = ["sh", "-c", "exec \"$0\" \"$@\"", root.bin, "-p", "--output-format", "json", "--tools", "", "--no-session-persistence", prompt];
        proc.running = true;
        guard.restart();
        return true;
    }

    function cancel() {
        if (!root.busy)
            return;
        root.stopReason = "cancelled";
        root.stopped = true;
        proc.running = false;
    }

    property string stopReason: ""
    property bool stopped: false
    property bool outDone: false
    property bool exited: false
    property int code: 0
    property string out: ""
    property string err: ""

    function settle() {
        if (!root.outDone || !root.exited)
            return;
        guard.stop();
        root.busy = false;
        let ok = false, text = "", error = "";
        if (root.stopped) {
            error = root.stopReason;
        } else if (root.code === 127) {
            error = "claude is not installed (looked for \"" + root.bin + "\")";
        } else {
            const r = Logic.replyText(root.out);
            if (r.ok && root.code === 0) {
                ok = true;
                text = r.text;
            } else {
                const tail = root.err.trim().split("\n").slice(-2).join(" ");
                error = !r.ok && root.out.trim() !== "" ? r.error : (tail !== "" ? tail : (r.ok ? "claude exited with " + root.code : r.error));
                if (/log ?in|auth|credential|api key/i.test(error))
                    error = "claude is not logged in. run `claude` once in a terminal";
            }
        }
        root.stopped = false;
        root.error = error;
        root.reply = text;
        root.finished(ok, text, error);
    }

    Timer {
        id: guard
        interval: root.timeoutMs
        onTriggered: {
            root.stopReason = "claude took too long";
            root.stopped = true;
            proc.running = false;
        }
    }

    Process {
        id: proc
        onRunningChanged: if (running) {
            root.outDone = false;
            root.exited = false;
            root.out = "";
            root.err = "";
        }
        stdout: StdioCollector {
            onStreamFinished: {
                root.out = text;
                root.outDone = true;
                root.settle();
            }
        }
        stderr: StdioCollector {
            onStreamFinished: root.err = text
        }
        onExited: (exitCode, exitStatus) => {
            root.code = exitCode;
            root.exited = true;
            root.settle();
        }
    }
}
