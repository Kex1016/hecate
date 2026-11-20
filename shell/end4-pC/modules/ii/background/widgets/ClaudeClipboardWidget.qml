pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common.widgets.haiiro as H

// hecate: ~/clipboard · the newest clipboard entry and four things Claude can
// do with it: explain, to nix, to toml, translate. Nothing is sent until a
// chip is pressed, and then only this one entry (the first 4000 characters,
// shown below with `sent`). Images are never sent. The answer is yours to
// `copy`; nothing is pasted or run for you.
H.HWindow {
    id: root

    property bool live: true
    property bool showContext: false
    property string action: ""

    readonly property string entry: Cliphist.entries.length > 0 ? Cliphist.entries[0] : ""
    readonly property bool image: root.entry !== "" && Cliphist.entryIsImage(root.entry)
    readonly property string entryBody: root.entry !== "" && !root.image ? Cliphist.entryText(root.entry).slice(0, 4000) : ""
    readonly property string lang: (Quickshell.env("LANG") || "en").slice(0, 2)

    readonly property var actions: ({
        "explain": "Explain what this is in two or three plain sentences. Do not repeat it.",
        "to nix": "Convert this to an equivalent Nix expression. Reply with only the Nix code, no fences.",
        "to toml": "Convert this to equivalent TOML. Reply with only the TOML, no fences.",
        "translate": `Translate this into English. If it is already English, translate it into the language with code '${root.lang === "en" ? "hu" : root.lang}'. Reply with only the translation.`
    })

    label: "hecate: ~/clipboard"
    widget: true
    implicitWidth: 420
    note: runner.running ? `${root.action}…` : runner.error !== "" ? runner.error
        : root.image ? "an image: not sent"
        : root.entryBody !== "" ? `sends this entry: ${root.entryBody.length} characters` : "clipboard is empty"

    ClaudeRunner {
        id: runner
        system: root.actions[root.action] ?? ""
        input: root.entryBody
    }

    function pick(name) {
        if (root.entryBody === "" || runner.running) return;
        root.action = name;
        runner.run();
    }

    H.ListRow {
        Layout.fillWidth: true
        selected: true
        text: root.image ? "[image]" : root.entryBody.replace(/\s+/g, " ").slice(0, 60)
    }

    Flow {
        Layout.fillWidth: true
        spacing: 6
        Repeater {
            model: ["explain", "to nix", "to toml", "translate"]
            H.Chip {
                required property string modelData
                text: modelData
                on: root.action === modelData && (runner.running || runner.done)
                interactive: root.entryBody !== ""
                onClicked: root.pick(modelData)
            }
        }
    }

    H.HText {
        Layout.fillWidth: true
        visible: runner.done && runner.error === "" && runner.result !== ""
        role: "code"
        ink: "c4"
        wrapMode: Text.WordWrap
        maximumLineCount: 12
        elide: Text.ElideRight
        text: runner.result
    }

    H.CodeBlock {
        Layout.fillWidth: true
        visible: root.showContext && root.entryBody !== ""
        lines: root.entryBody.split("\n").slice(0, 8).map(l => "# " + l.slice(0, 80))
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        visible: root.entryBody !== ""
        H.InkButton {
            visible: runner.done && runner.error === "" && runner.result !== ""
            ink: "c3"
            text: "copy"
            onClicked: Quickshell.clipboardText = runner.result
        }
        H.InkButton {
            visible: runner.done
            ink: "c1"
            text: "dismiss"
            onClicked: runner.reset()
        }
        H.Chip {
            text: "sent"
            on: root.showContext
            interactive: true
            onClicked: root.showContext = !root.showContext
        }
    }
}
