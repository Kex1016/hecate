import QtQuick
import qs.modules.common.widgets.haiiro

// A square progress bar: label and a tabular count above a 10px hairline
// track filled with text colour. Without a label or count it is the bare
// track (the OSD). Stack several in a Column with spacing 10.
//   Progress { label: "Main story"; count: "7 / 9"; value: 7 / 9 }
Item {
    id: root

    property string label: ""
    property string count: ""
    property real value: 0
    readonly property real position: Math.max(0, Math.min(1, value))

    implicitWidth: 360
    implicitHeight: col.implicitHeight

    Column {
        id: col
        width: root.width
        spacing: head.visible ? HTokens.space1 : 0

        Item {
            id: head
            visible: root.label.length > 0 || root.count.length > 0
            width: root.width
            height: Math.max(name.implicitHeight, num.implicitHeight)

            HText {
                id: name
                text: root.label
                role: "code"
                elide: Text.ElideRight
                width: parent.width - num.implicitWidth - 8
            }
            HText {
                id: num
                anchors.right: parent.right
                text: root.count
                role: "code"
                ink: "inkMuted"
            }
        }

        Item {
            width: root.width
            height: 10 + 2 * HTokens.borderHairline

            Rectangle {
                anchors.fill: parent
                color: HTokens.inkTrack
                border.width: HTokens.borderHairline
                border.color: HTokens.text
            }
            Rectangle {
                x: HTokens.borderHairline
                y: HTokens.borderHairline
                width: Math.round((parent.width - 2 * HTokens.borderHairline) * root.position)
                height: parent.height - 2 * HTokens.borderHairline
                color: HTokens.text
            }
        }
    }
}
