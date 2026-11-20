pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H

// cat config.json · change a setting by saying what you want. The sentence and
// the shell's current settings (a flat list; keys that look like secrets are
// left out, see `sent`) go to Claude, which names one setting and its new
// value. What you see is a diff computed here from the real current value, not
// Claude's own words. `apply` is the only thing that changes anything, and it
// is never pressed for you.
H.HWindow {
    id: root

    property bool live: true
    property bool showContext: false
    property var flat: ({})
    property string key: ""
    property var value: null
    property string appliedNote: ""
    property string problem: ""

    readonly property string configPath: FileUtils.trimFileProtocol(Directories.shellConfigPath)
    readonly property string configName: root.configPath.split("/").pop()
    readonly property var secretLike: /key|token|secret|password|passwd|credential|api/i
    readonly property var keys: Object.keys(root.flat).filter(k => !root.secretLike.test(k)).sort()
    readonly property string sentSettings: root.keys.map(k => `${k} = ${JSON.stringify(root.flat[k])}`).join("\n")
    readonly property bool proposing: root.key !== ""
    readonly property string oldText: JSON.stringify(root.flat[root.key]) ?? ""
    readonly property string newText: JSON.stringify(root.value) ?? ""

    label: `cat ${root.configName}`
    widget: true
    implicitWidth: 420
    note: runner.running ? "asking…" : root.problem !== "" ? root.problem : root.appliedNote !== "" ? root.appliedNote : root.proposing ? "nothing changes until you press apply" : "say it in plain words, then enter"

    function flatten(o, prefix, out) {
        for (const k of Object.keys(o)) {
            const v = o[k];
            const path = prefix === "" ? k : `${prefix}.${k}`;
            if (v !== null && typeof v === "object" && !Array.isArray(v)) root.flatten(v, path, out);
            else if (typeof v === "boolean" || typeof v === "number" || typeof v === "string") out[path] = v;
        }
        return out;
    }

    function ask() {
        const text = prompt.text.trim();
        if (text === "" || runner.running) return;
        root.key = "";
        root.problem = "";
        root.appliedNote = "";
        runner.run();
    }

    function clearProposal() {
        root.key = "";
        root.value = null;
        prompt.text = "";
    }

    FileView {
        id: configFile
        path: root.configPath
        onLoaded: {
            try { root.flat = root.flatten(JSON.parse(configFile.text()), "", {}); } catch (e) { root.flat = {}; }
        }
    }

    ClaudeRunner {
        id: runner
        system: "You change one setting of a desktop shell. The user says what they want; the list below is every setting as `key = json value`. Reply with exactly one line `key = json value` using a key from the list and a value of the same type, or `none: why` if no listed setting does it. Nothing else."
        input: `settings:\n${root.sentSettings}\n\nrequest: ${prompt.text.trim()}`
        onFinished: result => {
            const m = result.split("\n")[0].match(/^\s*([\w.\-]+)\s*=\s*(.+?)\s*$/);
            if (!m) { root.problem = result.split("\n")[0].toLowerCase() || "no setting matched"; return; }
            let v;
            try { v = JSON.parse(m[2]); } catch (e) { root.problem = "claude's answer was not a value"; return; }
            if (!(m[1] in root.flat) || root.secretLike.test(m[1])) { root.problem = `no setting called ${m[1]}`; return; }
            if (typeof v !== typeof root.flat[m[1]]) { root.problem = "claude proposed the wrong kind of value"; return; }
            root.value = v;
            root.key = m[1];
        }
        onErrorChanged: if (error !== "") root.problem = error
    }

    H.PromptLine {
        id: prompt
        Layout.fillWidth: true
        path: "~/settings"
        input: true
        onAccepted: root.ask()
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.proposing
        lines: [`- ${root.key} = ${root.oldText}`, `+ ${root.key} = ${root.newText}`]
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext
        lines: [`# request: ${prompt.text.trim()}`, `# and ${root.keys.length} settings, key = value`].concat(root.sentSettings.split("\n").slice(0, 12).map(l => "# " + l)).concat(root.keys.length > 12 ? ["# …"] : [])
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        H.InkButton {
            visible: root.proposing
            ink: "c3"
            text: "apply"
            onClicked: {
                Config.setNestedValue(root.key, root.value);
                root.appliedNote = `${root.key} is now ${root.newText}`;
                root.clearProposal();
                configFile.reload();
            }
        }
        H.InkButton {
            visible: root.proposing
            ink: "c1"
            text: "no"
            onClicked: root.clearProposal()
        }
        H.InkButton {
            visible: !root.proposing
            ink: "c3"
            text: runner.running ? "asking…" : "ask"
            enabled: !runner.running && prompt.text.trim() !== ""
            onClicked: root.ask()
        }
        H.Chip {
            text: "sent"
            on: root.showContext
            interactive: true
            onClicked: root.showContext = !root.showContext
        }
    }
}
