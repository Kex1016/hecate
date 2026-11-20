pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro as H

// The settings search: `@ ~/settings » grep ▮`, what you type after the
// command, with the kit's full block cursor.
Item {
    id: root

    property alias text: field.text
    signal keyPressed(var event)

    implicitWidth: line.implicitWidth + H.HTokens.ch() + Math.max(H.HTokens.ch(), field.contentWidth + H.HTokens.ch())
    implicitHeight: line.implicitHeight

    function forceInputFocus() {
        field.forceActiveFocus();
        field.cursorPosition = field.text.length;
    }

    H.PromptLine {
        id: line
        path: "~/settings"
        command: "grep"
    }

    TextInput {
        id: field
        x: line.implicitWidth + H.HTokens.ch()
        width: Math.max(0, root.width - x)
        height: line.implicitHeight
        verticalAlignment: TextInput.AlignVCenter
        color: H.HTokens.text
        selectionColor: H.HTokens.pine
        selectedTextColor: H.HTokens.text
        renderType: Text.NativeRendering
        font.family: H.HTokens.mono
        font.pixelSize: H.HTokens.fontBody
        font.kerning: false
        font.hintingPreference: Font.PreferFullHinting
        clip: true
        Keys.onPressed: event => root.keyPressed(event)
        // the block stays on screen as the prompt's cursor, like the canvas
        cursorDelegate: Rectangle {
            width: H.HTokens.ch()
            height: field.height
            color: H.HTokens.text
        }
        cursorVisible: true
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.IBeamCursor
        onClicked: root.forceInputFocus()
    }
}
