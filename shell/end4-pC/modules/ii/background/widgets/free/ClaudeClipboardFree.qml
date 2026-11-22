pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H
import ".." as W

// wl-paste | claude explain, free · a pixel clipboard and the newest entry in foam, Claude's
// answer in rose, and four things to do with it as ink words: explain, to nix, to toml,
// translate. Nothing is sent until one is pressed, and then only this entry (the first 4000
// characters, print them with `sent`). Images are never sent. The answer is yours to `copy`;
// nothing is pasted or run for you.
H.FreeWidget {
    id: root

    property bool showContext: false
    property string action: ""

    readonly property string entry: Cliphist.entries.length > 0 ? Cliphist.entries[0] : ""
    readonly property bool image: root.entry !== "" && Cliphist.entryIsImage(root.entry)
    readonly property string entryBody: root.entry !== "" && !root.image ? Cliphist.entryText(root.entry).slice(0, 4000) : ""
    readonly property string lang: (Quickshell.env("LANG") || "en").slice(0, 2)
    readonly property bool answered: runner.done && runner.error === "" && runner.result !== ""

    readonly property var actions: ({
        "explain": "Explain what this is in two or three plain sentences. Do not repeat it.",
        "to nix": "Convert this to an equivalent Nix expression. Reply with only the Nix code, no fences.",
        "to toml": "Convert this to equivalent TOML. Reply with only the TOML, no fences.",
        "translate": `Translate this into English. If it is already English, translate it into the language with code '${root.lang === "en" ? "hu" : root.lang}'. Reply with only the translation.`
    })

    label: "wl-paste | claude explain"
    implicitWidth: 400
    spacing: 10

    function pick(name) {
        if (root.entryBody === "" || runner.running) return;
        root.action = name;
        runner.run();
    }

    W.ClaudeRunner {
        id: runner
        system: root.actions[root.action] ?? ""
        input: root.entryBody
    }

    Row {
        spacing: 14
        H.PixelGlyph {
            id: glyph
            name: "clipboard"
        }
        H.FreeText {
            width: root.width - glyph.width - 14
            anchors.verticalCenter: parent.verticalCenter
            role: "code"
            leading: H.HTokens.leadingPre
            ink: "foam"
            elide: Text.ElideRight
            text: root.image ? "[image]" : root.entryBody !== "" ? root.entryBody.replace(/\s+/g, " ").slice(0, 120) : "clipboard is empty"
        }
    }

    H.FreeText {
        Layout.fillWidth: true
        visible: root.answered
        size: 16
        leading: H.HTokens.leadingProse
        wrapMode: Text.WordWrap
        maximumLineCount: 12
        elide: Text.ElideRight
        ink: "rose"
        text: runner.result
    }

    H.FreeText {
        visible: runner.running || runner.error !== "" || (!root.answered && root.entryBody !== "") || root.image
        role: "caption"
        ink: runner.error !== "" ? "love" : "muted"
        text: runner.running ? `${root.action}…` : runner.error !== "" ? runner.error : root.image ? "an image: not sent" : `sends this entry: ${root.entryBody.length} characters`
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.entryBody !== ""
        lines: root.entryBody.split("\n").slice(0, 8).map(l => "# " + l.slice(0, 80))
    }

    Row {
        spacing: 4
        Repeater {
            model: ["explain", "to nix", "to toml", "translate"]
            H.InkButton {
                required property string modelData
                readonly property bool current: root.action === modelData && (runner.running || runner.done)
                ink: current ? "c5" : ""
                flat: !current
                on: false
                text: modelData
                enabled: root.entryBody !== ""
                onClicked: root.pick(modelData)
            }
        }
    }

    Row {
        spacing: 4
        visible: root.answered || runner.done || root.entryBody !== ""
        H.InkButton {
            visible: root.answered
            ink: "c3"
            text: "copy"
            onClicked: Quickshell.clipboardText = runner.result
        }
        H.InkButton {
            visible: runner.done
            flat: true
            role: "caption"
            text: "dismiss"
            onClicked: runner.reset()
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
