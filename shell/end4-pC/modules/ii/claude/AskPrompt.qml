pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro as H

// The question line: `@ ~ » ? why does this fail`, the PromptLine colours
// with a gold `?` standing in for the command, and the full block cursor.
Item {
    id: root

    property alias text: field.text
    property alias field: field
    signal accepted()
    // ctrl enter: the keyboard's gold button
    signal runRequested()
    // after an answer: the first key typed starts a new question
    property bool replaceOnType: false

    readonly property int size: H.HTokens.fontBody
    readonly property int gap: H.HTokens.ch(size)

    implicitWidth: line.implicitWidth + gap + field.contentWidth + gap
    implicitHeight: H.HTokens.lineBox(size)

    function focusField() {
        field.forceActiveFocus();
    }

    Row {
        id: line
        spacing: root.gap
        H.HText { text: "@"; ink: "love" }
        H.HText { text: "~"; ink: "foam" }
        H.HText { text: "»"; ink: "rose" }
        H.HText { text: "?"; ink: "gold" }
    }

    TextInput {
        id: field
        x: line.implicitWidth + root.gap
        width: Math.max(0, root.width - x)
        height: root.implicitHeight
        verticalAlignment: TextInput.AlignVCenter
        color: H.HTokens.text
        selectionColor: H.HTokens.pine
        selectedTextColor: H.HTokens.text
        renderType: Text.NativeRendering
        font.family: H.HTokens.mono
        font.pixelSize: root.size
        font.kerning: false
        font.hintingPreference: Font.PreferFullHinting
        clip: true
        onAccepted: root.accepted()
        Keys.onPressed: event => {
            if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && (event.modifiers & Qt.ControlModifier)) {
                root.runRequested();
                event.accepted = true;
            } else if (root.replaceOnType && event.text.length > 0 && event.text.charCodeAt(0) >= 32
                       && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier))) {
                field.text = "";
            }
            if (event.key !== Qt.Key_Shift)
                root.replaceOnType = false;
        }
        cursorDelegate: Rectangle {
            width: H.HTokens.ch(root.size)
            height: root.field.height
            color: H.HTokens.text
            visible: root.field.activeFocus
        }
    }
}
