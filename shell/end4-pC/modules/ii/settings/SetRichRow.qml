import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// A SetRow whose name is styled text, for rows that colour part of their
// name: an account's kind, a calendar's ■. Same layout as the kit's
// SettingRow: hairline above, name left, control right, description under.
SetRow {
    id: root

    // StyledText: <font color="#f6c177">icloud</font> me@icloud.com
    property string richName: ""
    property alias extra: extraSlot.data

    name: richName.replace(/<[^>]+>/g, "")
    plainLayout: false
    implicitHeight: grid.implicitHeight + 16 + H.HTokens.borderHairline

    function ink(token, text) {
        return `<font color="${H.HTokens.ink(token)}">${text}</font>`;
    }

    data: [
    Rectangle {
        width: parent.width
        height: H.HTokens.borderHairline
        color: H.HTokens.inkHairline
    },
    GridLayout {
        id: grid
        y: 8 + H.HTokens.borderHairline
        width: parent.width
        columns: 2
        columnSpacing: 24
        rowSpacing: 2
        opacity: root.enabled ? 1 : 0.55

        H.HText {
            Layout.fillWidth: true
            text: root.richName
            textFormat: Text.StyledText
            elide: Text.ElideRight
        }
        Row {
            id: extraSlot
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            spacing: 12
        }
        H.HText {
            visible: root.description.length > 0
            Layout.fillWidth: true
            text: root.description
            role: "caption"
            ink: "subtle"
            wrapMode: Text.Wrap
        }
    }
    ]
}
