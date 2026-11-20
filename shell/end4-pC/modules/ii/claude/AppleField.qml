import QtQuick
import qs.modules.common.widgets.haiiro as H

// One labelled line of an edit form: `when  2026-11-21T20:00`.
Item {
    id: root

    property string label: ""
    property string value: ""
    property bool invalid: false
    property alias field: input
    signal committed(string text)

    implicitHeight: H.HTokens.lineBox(H.HTokens.role("body").size)
    implicitWidth: 320

    H.HText {
        id: name
        width: H.HTokens.ch() * 8
        text: root.label
        ink: "gold"
    }

    TextInput {
        id: input
        x: name.width
        width: root.width - x
        height: root.height
        text: root.value
        verticalAlignment: TextInput.AlignVCenter
        color: root.invalid ? H.HTokens.love : H.HTokens.text
        selectionColor: H.HTokens.pine
        selectedTextColor: H.HTokens.text
        renderType: Text.NativeRendering
        font.family: H.HTokens.mono
        font.pixelSize: H.HTokens.role("body").size
        clip: true
        onEditingFinished: root.committed(text)
        Keys.onEscapePressed: event => { text = root.value; event.accepted = false; }
    }

    Rectangle {
        x: name.width
        y: root.height - 1
        width: root.width - x
        height: H.HTokens.borderHairline
        color: input.activeFocus ? H.HTokens.foam : H.HTokens.inkHairline
    }
}
