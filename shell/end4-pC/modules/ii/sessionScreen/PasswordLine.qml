import QtQuick
import qs.modules.common.widgets.haiiro as H

// A password prompt the way a terminal draws it: the prompt words, then
// nothing (sudo) or one `*` per key (polkit), then a full block cursor. The
// real input is an invisible TextInput that holds focus.
//
//   PasswordLine {
//       stars: true
//       H.HText { text: "@"; ink: "love" }
//   }
Item {
    id: root
    // prompt words, laid out left to right before the typed text
    default property alias prompt: words.data
    property alias text: field.text
    property alias echoMode: field.echoMode
    property bool stars: false
    property bool busy: false
    signal accepted()
    signal cancelled()

    implicitWidth: line.implicitWidth
    implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody)

    function forceInputFocus() { field.forceActiveFocus(); }
    function clear() { field.text = ""; }

    Row {
        id: line
        spacing: H.HTokens.ch()
        Row {
            id: words
            spacing: H.HTokens.ch()
        }
        H.HText {
            visible: root.stars && field.text.length > 0
            text: "*".repeat(field.text.length)
            ink: "iris"
            font.letterSpacing: 2
        }
        Rectangle {
            visible: field.activeFocus
            width: H.HTokens.ch()
            height: H.HTokens.lineBox(H.HTokens.fontBody)
            color: H.HTokens.text
        }
    }

    TextInput {
        id: field
        width: 1
        height: 1
        opacity: 0
        enabled: !root.busy
        echoMode: TextInput.Password
        focus: true
        onAccepted: root.accepted()
        Keys.onEscapePressed: root.cancelled()
    }

    MouseArea {
        anchors.fill: parent
        onClicked: field.forceActiveFocus()
    }
}
