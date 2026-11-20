import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.services
import qs.modules.common
import qs.modules.common.widgets.haiiro as H

Item {
    id: root
    property bool shown: true
    signal closed()

    readonly property bool usePasswordChars: !(PolkitService.flow?.responseVisible ?? false)
    // the last answer was refused, until the next keystroke
    property bool refused: false
    // "polkit: authenticate" with what is being asked, and which action it is
    readonly property string actionId: PolkitService.flow?.actionId ?? ""
    readonly property string prompt: PolkitService.cleanPrompt.toLowerCase() + ":"

    function submit() {
        if (!PolkitService.interactionAvailable || line.text === "") return;
        root.refused = false;
        PolkitService.submit(line.text);
    }
    function deny() {
        PolkitService.cancel();
    }

    Keys.onEscapePressed: root.deny()

    Connections {
        target: PolkitService
        function onInteractionAvailableChanged() {
            if (!PolkitService.interactionAvailable) return;
            line.clear();
            line.forceInputFocus();
        }
    }
    Connections {
        target: PolkitService.flow
        function onAuthenticationFailed() {
            root.refused = true;
        }
    }

    H.PixelTransition {
        anchors.centerIn: parent
        width: win.implicitWidth
        height: win.implicitHeight
        shown: root.shown
        onClosed: root.closed()

        H.HWindow {
            id: win
            width: 520
            height: implicitHeight
            label: "polkit: authenticate"
            variant: "gold"
            hints: [
                { key: "enter", text: "allow" },
                { key: "esc", text: "deny" }
            ]

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 12
                H.HText {
                    Layout.fillWidth: true
                    text: "Something asks for more than you usually give it."
                    ink: "rose"
                    wrapMode: Text.Wrap
                }
                H.HText {
                    Layout.fillWidth: true
                    text: PolkitService.cleanMessage !== "" ? PolkitService.cleanMessage + "." : ""
                    ink: "iris"
                    wrapMode: Text.Wrap
                }
                H.HText {
                    Layout.fillWidth: true
                    visible: root.actionId !== ""
                    text: root.actionId
                    role: "label"
                    ink: "c1"
                    wrapMode: Text.WrapAnywhere
                }
                H.PasswordLine {
                    id: line
                    stars: root.usePasswordChars
                    echoMode: root.usePasswordChars ? TextInput.Password : TextInput.Normal
                    busy: !PolkitService.interactionAvailable
                    onAccepted: root.submit()
                    onCancelled: root.deny()
                    onTextChanged: root.refused = false
                    H.HText { text: "@"; ink: "love" }
                    H.HText { text: Quickshell.env("USER"); ink: "foam" }
                    H.HText { text: "»"; ink: "rose" }
                    H.HText { text: root.prompt; ink: "gold" }
                }
                H.HText {
                    Layout.fillWidth: true
                    visible: root.refused
                    text: "Wrong password. Try again."
                    ink: "love"
                }
                RowLayout {
                    spacing: 8
                    H.InkButton {
                        text: "allow"
                        ink: "c3"
                        enabled: PolkitService.interactionAvailable && line.text !== ""
                        onClicked: root.submit()
                    }
                    H.InkButton {
                        text: "deny"
                        ink: "c2"
                        onClicked: root.deny()
                    }
                }
            }
        }
    }

    Component.onCompleted: line.forceInputFocus()
}
