import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// "us » jp": the layout you left in muted ink, an arrow, the one you are on.
H.HWindow {
    id: root
    property string previous: ""
    property string current: ""

    width: H.HTokens.osdWidth
    label: "keyboard"

    RowLayout {
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignHCenter
        spacing: H.HTokens.ch()

        Item { Layout.fillWidth: true }
        H.HText {
            visible: root.previous.length > 0
            text: root.previous
            ink: "c1"
        }
        H.HText {
            visible: root.previous.length > 0
            text: "»"
            ink: "c4"
        }
        H.HText {
            text: root.current
            ink: "c6"
        }
        Item { Layout.fillWidth: true }
    }
}
