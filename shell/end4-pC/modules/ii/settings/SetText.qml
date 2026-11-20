import QtQuick
import qs.modules.common.widgets.haiiro as H

// A line of text: what you type follows a dim `=` on a hairline, saved when
// you press enter or leave the field.
//   SetText {
//       name: "name"
//       key: "profile.displayName"
//       placeholder: "your user name"
//   }
SetRow {
    id: root

    property string placeholder: ""
    property int columns: 24
    property bool secret: false
    // a field that adds things (a new alias): empty again after enter
    property bool clearOnCommit: false
    // what is in the field right now, saved or not
    readonly property alias text: field.text
    signal accepted()

    // Sets the value and the field, even over what was typed.
    function reset(v) {
        root.value = v;
        field.text = String(v ?? "");
    }
    onCurrentChanged: if (!field.activeFocus) field.text = String(root.current ?? "")

    Item {
        implicitWidth: H.HTokens.ch() * root.columns
        implicitHeight: H.HTokens.lineBox(H.HTokens.fontBody) + 2

        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: H.HTokens.borderHairline
            color: field.activeFocus ? H.HTokens.foam : H.HTokens.inkHairline
        }
        H.HText {
            visible: field.text.length === 0
            text: root.placeholder
            ink: "muted"
            elide: Text.ElideRight
            width: parent.width
        }
        TextInput {
            id: field
            anchors.fill: parent
            anchors.bottomMargin: 2
            activeFocusOnTab: true
            text: String(root.current ?? "")
            echoMode: root.secret ? TextInput.Password : TextInput.Normal
            color: H.HTokens.text
            selectionColor: H.HTokens.pine
            selectedTextColor: H.HTokens.text
            renderType: Text.NativeRendering
            font.family: H.HTokens.mono
            font.pixelSize: H.HTokens.fontBody
            font.kerning: false
            font.hintingPreference: Font.PreferFullHinting
            verticalAlignment: TextInput.AlignVCenter
            clip: true
            onAccepted: root.accepted()
            onEditingFinished: {
                if (text !== String(root.current ?? "")) root.write(text);
                if (root.clearOnCommit) text = "";
            }
            Keys.onEscapePressed: event => {
                text = String(root.current ?? "");
                event.accepted = false;
            }
        }
    }
}
