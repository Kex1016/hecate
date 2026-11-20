pragma ComponentBehavior: Bound
import qs.services.claude
import qs.modules.common.widgets.haiiro as H
import QtQuick
import QtQuick.Layouts
import "AskParse.js" as Ask

// The answer, in author voice: paragraphs with `code` in iris, commands in
// code blocks, then the buttons. A command runs only from the gold button
// (or ctrl enter); with several, the chosen one has the rose » and a click
// on another block chooses it. Scrolls when it outgrows `maxHeight`.
ColumnLayout {
    id: root

    property int maxHeight: 560

    spacing: 10

    // a dot every 250ms while waiting, stepped like everything else
    property int dots: 0
    Timer {
        running: ClaudeAsk.phase === "asking" && root.visible
        interval: 250
        repeat: true
        onTriggered: root.dots = (root.dots + 1) % 4
    }

    Flickable {
        id: flick
        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(col.implicitHeight, root.maxHeight)
        contentWidth: width
        contentHeight: col.implicitHeight
        clip: contentHeight > height
        interactive: contentHeight > height
        boundsBehavior: Flickable.StopAtBounds

        // keep the newest words in view while they arrive
        onContentHeightChanged: if (ClaudeAsk.busy && contentHeight > height) contentY = contentHeight - height

        Column {
            id: col
            width: flick.width
            spacing: 10

            // not set up: the opt-in
            Column {
                visible: !ClaudeAsk.available && ClaudeAsk.checked
                width: parent.width
                spacing: 4
                H.HText {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: "Claude Code is not set up on this machine."
                }
                H.HText {
                    width: parent.width
                    wrapMode: Text.Wrap
                    role: "caption"
                    ink: "inkMuted"
                    text: "The ask panel runs the claude command on your own login. Turn on the Claude tools in CakeOS (or install Claude Code), run claude once in a terminal to log in, then press Mod+/ again. Nothing was sent."
                }
            }

            H.HText {
                visible: ClaudeAsk.phase === "asking"
                text: "claude is thinking" + ".".repeat(root.dots)
                ink: "rose"
            }

            Repeater {
                model: ClaudeAsk.available ? ClaudeAsk.blocks : []

                Item {
                    id: block
                    required property var modelData
                    required property int index
                    readonly property bool isCode: modelData.kind === "code"
                    readonly property int commandAt: modelData.command ?? -1
                    readonly property bool chosen: commandAt >= 0 && commandAt === ClaudeAsk.commandIndex
                    readonly property bool marked: chosen && ClaudeAsk.commands.length > 1
                    readonly property int gutter: ClaudeAsk.commands.length > 1 && isCode ? 2 * H.HTokens.ch() : 0

                    width: col.width
                    height: isCode ? code.height : para.height

                    H.HText {
                        id: para
                        visible: !block.isCode
                        width: parent.width
                        wrapMode: Text.Wrap
                        textFormat: Text.StyledText
                        text: block.isCode ? "" : Ask.styled(block.modelData.text, H.HTokens.iris)
                    }

                    H.HText {
                        visible: block.marked
                        y: 6
                        text: "»"
                        ink: "rose"
                    }

                    H.CodeBlock {
                        id: code
                        visible: block.isCode
                        x: block.gutter
                        width: parent.width - block.gutter
                        lines: block.isCode && block.modelData.runnable ? block.modelData.lines : []

                        // code that is not a command: plain, copy only
                        Repeater {
                            model: block.isCode && !block.modelData.runnable ? block.modelData.lines : []
                            H.HText {
                                required property string modelData
                                width: code.width - 20
                                text: modelData.length > 0 ? modelData : " "
                                role: "code"
                                wrapMode: Text.WrapAnywhere
                            }
                        }
                    }

                    // a click chooses the command the gold button runs
                    MouseArea {
                        anchors.fill: code
                        enabled: block.isCode && block.commandAt >= 0
                        cursorShape: Qt.PointingHandCursor
                        onClicked: ClaudeAsk.commandIndex = block.commandAt
                    }
                }
            }

            // failed: what to do about it, then claude's own words
            Column {
                visible: ClaudeAsk.phase === "error"
                width: parent.width
                spacing: 4
                H.HText {
                    width: parent.width
                    wrapMode: Text.Wrap
                    ink: "love"
                    text: ClaudeAsk.errorHint.length > 0 ? ClaudeAsk.errorHint : "Claude could not answer."
                }
                H.HText {
                    width: parent.width
                    wrapMode: Text.WrapAnywhere
                    role: "caption"
                    ink: "inkMuted"
                    text: ClaudeAsk.error
                }
            }
        }
    }

    Row {
        Layout.fillWidth: true
        spacing: 8
        visible: (ClaudeAsk.command !== null && ClaudeAsk.phase !== "error") || ClaudeAsk.busy || (ClaudeAsk.phase === "done" && ClaudeAsk.sessionId.length > 0)

        // the one button that acts
        H.InkButton {
            visible: ClaudeAsk.command !== null && ClaudeAsk.phase !== "error"
            enabled: ClaudeAsk.phase === "done"
            ink: "c3"
            text: `run in ${ClaudeAsk.terminalName}`
            onClicked: ClaudeAsk.runCommand()
        }
        H.InkButton {
            visible: ClaudeAsk.command !== null && ClaudeAsk.phase !== "error"
            ink: "c1"
            text: "copy"
            onClicked: ClaudeAsk.copyCommand()
        }
        H.InkButton {
            visible: ClaudeAsk.busy
            ink: "c1"
            text: "stop"
            onClicked: ClaudeAsk.stop()
        }
        H.InkButton {
            visible: ClaudeAsk.phase === "done" && ClaudeAsk.sessionId.length > 0 && ClaudeAsk.available
            ink: "c5"
            text: "continue in claude code"
            onClicked: ClaudeAsk.continueInClaudeCode()
        }
    }
}
