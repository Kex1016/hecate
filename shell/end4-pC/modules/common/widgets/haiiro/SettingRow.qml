import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// One setting: the name on the left, its control on the right (a Chip,
// Segmented or Spin as the child), an optional subtle description under the
// name, a hairline above. A setting that does not apply right now is
// `enabled: false`: it drops to 55% opacity rather than disappearing.
//   SettingRow { name: "background"; description: "Blobs: the live simulation."
//                Segmented { model: ["blobs", "wallpaper"] } }
Item {
    id: root

    property string name: ""
    property string description: ""
    default property alias control: slot.data

    implicitWidth: grid.implicitWidth
    implicitHeight: grid.implicitHeight + 16 + HTokens.borderHairline
    opacity: enabled ? 1 : 0.55

    Rectangle {
        width: parent.width
        height: HTokens.borderHairline
        color: HTokens.inkHairline
    }

    GridLayout {
        id: grid
        y: 8 + HTokens.borderHairline
        width: parent.width
        columns: 2
        columnSpacing: 24
        rowSpacing: 2

        HText {
            Layout.fillWidth: true
            text: root.name
            elide: Text.ElideRight
        }
        Item {
            id: slot
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            Layout.rowSpan: 1
            implicitWidth: childrenRect.width
            implicitHeight: childrenRect.height
        }
        HText {
            visible: root.description.length > 0
            Layout.fillWidth: true
            Layout.columnSpan: 1
            text: root.description
            role: "caption"
            ink: "subtle"
            wrapMode: Text.Wrap
        }
    }
}
