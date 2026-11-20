pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro as H

// The launcher's search field: `@ ~ » fir█`, or with a prefix in use
// `@ ~/clipboard » grep █`. The prefix itself is never in the field; the
// path and the gold command show which mode is on.
Item {
    id: root

    property string path: "~"
    property string command: ""
    property string role: "nav"
    property alias field: field
    property alias text: field.text

    // typed (not set by code), and every key the field sees first
    signal edited()
    signal keyPressed(var event)

    readonly property int size: H.HTokens.role(role).size
    readonly property int gap: H.HTokens.ch(size)

    implicitWidth: line.implicitWidth + root.gap + field.contentWidth + H.HTokens.ch(size)
    implicitHeight: H.HTokens.lineBox(size)

    Row {
        id: line
        height: parent.height
        spacing: root.gap
        H.HText { text: "@"; ink: "love"; role: root.role }
        H.HText { text: root.path; ink: "foam"; role: root.role; visible: root.path.length > 0 }
        H.HText { text: "»"; ink: "rose"; role: root.role }
        H.HText { text: root.command; ink: "gold"; role: root.role; visible: root.command.length > 0 }
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
        onTextEdited: root.edited()
        Keys.onPressed: event => root.keyPressed(event)
        // a full block, 1ch by one line, that never blinks
        cursorDelegate: Rectangle {
            width: H.HTokens.ch(root.size)
            height: field.height
            color: H.HTokens.text
        }
    }
}
