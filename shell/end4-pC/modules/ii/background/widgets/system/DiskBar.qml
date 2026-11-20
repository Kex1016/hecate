import QtQuick
import qs.modules.common.widgets.haiiro as H

// H.Progress with a love variant, for a disk that is nearly full: label, count, track
// border and fill all turn love.
Item {
    id: root

    property string label: ""
    property string count: ""
    property real value: 0
    property bool full: false
    readonly property real position: Math.max(0, Math.min(1, value))
    readonly property color ink: full ? H.HTokens.love : H.HTokens.text

    implicitWidth: 360
    implicitHeight: col.implicitHeight

    Column {
        id: col
        width: root.width
        spacing: H.HTokens.space1

        Item {
            width: root.width
            height: Math.max(name.implicitHeight, num.implicitHeight)

            H.HText {
                id: name
                width: parent.width - num.implicitWidth - 8
                role: "code"
                ink: root.full ? "c2" : "text"
                elide: Text.ElideMiddle
                text: root.label
            }
            H.HText {
                id: num
                anchors.right: parent.right
                role: "code"
                ink: root.full ? "c2" : "inkMuted"
                text: root.count
            }
        }

        Item {
            width: root.width
            height: 10 + 2 * H.HTokens.borderHairline

            Rectangle {
                anchors.fill: parent
                color: H.HTokens.inkTrack
                border.width: H.HTokens.borderHairline
                border.color: root.ink
            }
            Rectangle {
                x: H.HTokens.borderHairline
                y: H.HTokens.borderHairline
                width: Math.round((parent.width - 2 * H.HTokens.borderHairline) * root.position)
                height: parent.height - 2 * H.HTokens.borderHairline
                color: root.ink
            }
        }
    }
}
