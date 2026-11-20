import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// One cell of the dashboard's stat panel: a 1px text-coloured frame, 16px in,
// an uppercase heading with an optional muted sub line or badge after it,
// and the tile's lines under it.
//   DashTile { title: "network"; sub: "wlan0 · 72%"; H.HText { ... } }
Item {
    id: root

    property string title: ""
    property string sub: ""
    property string badge: ""
    // the heading and the lines share a column; set false for a tile with art beside it
    property bool headed: true
    property int spacing: 8
    property alias heading: headingRow
    default property alias content: body.data
    // a tile that leads somewhere: hover lights the frame foam, a click fires
    property bool interactive: false
    signal clicked()

    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.preferredWidth: 1
    implicitHeight: col.implicitHeight + 2 * H.HTokens.space4

    MouseArea {
        anchors.fill: parent
        enabled: root.interactive
        hoverEnabled: root.interactive
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
        id: mouse
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: H.HTokens.borderHairline
        border.color: mouse.containsMouse ? H.HTokens.foam : H.HTokens.text
    }

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: H.HTokens.space4
        spacing: root.spacing

        Row {
            id: headingRow
            visible: root.headed
            spacing: H.HTokens.space2
            Layout.bottomMargin: 0

            H.HText {
                text: root.title
                role: "headingSm"
            }
            H.HText {
                visible: root.sub.length > 0
                anchors.verticalCenter: parent.verticalCenter
                text: root.sub
                role: "caption"
                ink: "inkMuted"
            }
            H.Chip {
                visible: root.badge.length > 0
                anchors.verticalCenter: parent.verticalCenter
                badge: true
                text: root.badge
            }
        }

        ColumnLayout {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: root.spacing
        }
    }
}
