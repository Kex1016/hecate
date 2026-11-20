import QtQuick
import Quickshell
import Quickshell.Io
import qs.services.claude

// One `claude -p` call, started by `run()` and nothing else. Claude Code's own
// login is used (no key is read or stored here) and every built-in tool is
// switched off, so the model can only answer: it cannot read files or run
// anything. `input` is the whole of what is sent on stdin; the widget that owns
// the runner shows the same text, so nothing leaves the machine unseen.
//
//   ClaudeRunner { id: r; system: "Explain in two sentences."; input: text }
//   onClicked: r.run()           r.result, r.error, r.running
//
// HECATE_CLAUDE_BIN swaps the executable (tests use a fake one).
Item {
    id: root

    property string system: ""
    property string input: ""
    property string result: ""
    property string error: ""
    property bool done: false
    readonly property bool running: proc.running
    signal finished(string result)

    function run() {
        if (proc.running) return;
        root.result = "";
        root.error = "";
        root.done = false;
        proc.command = [ClaudeBrief.binary, "-p", "--tools", "", "--no-session-persistence", "--output-format", "text", "--append-system-prompt", root.system];
        proc.running = true;
    }

    function cancel() {
        if (proc.running) proc.signal(15);
    }

    function reset() {
        root.result = "";
        root.error = "";
        root.done = false;
    }

    Process {
        id: proc
        stdinEnabled: true
        environment: ({ PATH: ClaudeBrief.toolPath, HECATE_CLAUDE_BRIDGE: "0" })
        onStarted: {
            write(root.input + "\n");
            stdinEnabled = false;
        }
        stdout: StdioCollector {
            onStreamFinished: root.result = text.trim()
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim().length > 0) root.error = text.trim().split("\n")[0].toLowerCase()
        }
        onExited: (code, status) => {
            stdinEnabled = true;
            if (code !== 0 && root.error === "") root.error = `claude exited with ${code}`;
            root.done = true;
            if (code === 0) root.finished(root.result);
        }
    }
}
