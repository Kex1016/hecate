pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// The fake shell prompt that opens a section: `@ ~/path » command args`,
// each part in its own ink (@ love, path foam, » rose, command gold,
// arguments iris). A bare script (`./stats.sh`) is `script`, iris alone.
// `input: true` turns it into the search field of a launcher or picker: what
// you type follows the », with a full block cursor (1ch by one line).
//   PromptLine { path: "~/about"; command: "cat"; args: "details.md" }
//   PromptLine { path: "~"; input: true; role: "nav"; onAccepted: ... }
Item {
    id: root

    property string path: "~"
    property string command: ""
    property string args: ""
    property string script: ""
    property bool input: false
    property string role: "body"
    property alias text: field.text
    property alias field: field
    signal accepted()

    readonly property int size: HTokens.role(role).size
    readonly property int gap: HTokens.ch(size)

    implicitWidth: line.implicitWidth + (input ? field.contentWidth + HTokens.ch(size) + gap : 0)
    implicitHeight: HTokens.lineBox(size)

    function forceInputFocus() { field.forceActiveFocus(); }

    Row {
        id: line
        spacing: root.gap
        HText { text: "@"; ink: "love"; role: root.role }
        HText { text: root.path; ink: "foam"; role: root.role; visible: root.path.length > 0 }
        HText { text: "»"; ink: "rose"; role: root.role }
        HText { text: root.command; ink: "gold"; role: root.role; visible: !root.input && root.command.length > 0 }
        HText { text: root.args; ink: "iris"; role: root.role; visible: !root.input && root.args.length > 0 }
        HText { text: root.script; ink: "iris"; role: root.role; visible: !root.input && root.script.length > 0 }
    }

    TextInput {
        id: field
        visible: root.input
        enabled: root.input
        x: line.implicitWidth + root.gap
        width: Math.max(0, root.width - x)
        height: root.implicitHeight
        verticalAlignment: TextInput.AlignVCenter
        color: HTokens.text
        selectionColor: HTokens.pine
        selectedTextColor: HTokens.text
        renderType: Text.NativeRendering
        font.family: HTokens.mono
        font.pixelSize: root.size
        font.kerning: false
        font.hintingPreference: Font.PreferFullHinting
        clip: true
        onAccepted: root.accepted()
        cursorDelegate: Rectangle {
            width: HTokens.ch(root.size)
            height: field.height
            color: HTokens.text
            visible: field.activeFocus || root.input
        }
    }
}
