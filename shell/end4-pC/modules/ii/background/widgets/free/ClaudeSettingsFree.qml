pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets.haiiro as H
import ".." as W

// claude: ~/settings, free · say what you want changed in plain words. The sentence and the shell's
// current settings (a flat list; keys that look like secrets are left out, print it with `sent`)
// go to Claude, which names one setting and its new value. What you see is a diff computed here
// from the real current value, not Claude's words, and for an on/off value the switch flipping
// in pixels. `apply` is the only thing that changes anything and is never pressed for you.
H.FreeWidget {
    id: root

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
    readonly property bool isSwitch: root.proposing && typeof root.value === "boolean"
    // the last two parts of the key, "bar · seconds"
    readonly property string niceKey: root.key.split(".").slice(-2).join(" · ")

    label: "claude: ~/settings"
    implicitWidth: 400
    spacing: 8

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

    W.ClaudeRunner {
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

    // the question: a gold ? and what you type, in text
    Row {
        spacing: 8
        H.FreeText { id: q; role: "code"; ink: "gold"; text: "?" }
        Item {
            width: root.width - q.width - 8
            height: prompt.height
            H.FreeText {
                visible: prompt.text === ""
                role: "code"
                ink: "muted"
                text: "say it in plain words, then enter"
            }
            TextInput {
                id: prompt
                width: parent.width
                color: H.HTokens.text
                font.family: H.HTokens.mono
                font.pixelSize: H.HTokens.fontHeadingSm
                selectionColor: H.HTokens.highlightMed
                clip: true
                onAccepted: root.ask()
            }
        }
    }

    H.FreeText {
        visible: root.proposing
        role: "code"
        ink: "subtle"
        text: root.configName
    }
    H.FreeText {
        visible: root.proposing
        role: "code"
        ink: "love"
        text: `- ${root.key} = ${root.oldText}`
    }
    H.FreeText {
        visible: root.proposing
        role: "code"
        ink: "foam"
        text: `+ ${root.key} = ${root.newText}`
    }

    Row {
        Layout.topMargin: 2
        visible: root.isSwitch
        spacing: 14
        H.PixelGlyph {
            anchors.verticalCenter: parent.verticalCenter
            name: root.flat[root.key] === true ? "toggleOn" : "toggleOff"
            cell: 4
        }
        H.FreeText { anchors.verticalCenter: parent.verticalCenter; role: "code"; ink: "subtle"; text: "→" }
        H.PixelGlyph {
            anchors.verticalCenter: parent.verticalCenter
            name: root.value === true ? "toggleOn" : "toggleOff"
            cell: 4
        }
        H.FreeText { anchors.verticalCenter: parent.verticalCenter; role: "caption"; ink: "muted"; text: root.niceKey }
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: !root.proposing && (runner.running || root.problem !== "" || root.appliedNote !== "")
        role: "caption"
        ink: root.problem !== "" ? "love" : "muted"
        elide: Text.ElideRight
        text: runner.running ? "asking…" : root.problem !== "" ? root.problem : root.appliedNote
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext
        lines: [`# request: ${prompt.text.trim()}`, `# and ${root.keys.length} settings, key = value`].concat(root.sentSettings.split("\n").slice(0, 12).map(l => "# " + l)).concat(root.keys.length > 12 ? ["# …"] : [])
    }

    Row {
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
        H.InkButton {
            flat: true
            role: "caption"
            text: "sent"
            on: root.showContext
            onClicked: root.showContext = !root.showContext
        }
    }
    H.FreeText {
        visible: root.proposing
        role: "caption"
        ink: "muted"
        text: "nothing changes until you press apply"
    }
}
