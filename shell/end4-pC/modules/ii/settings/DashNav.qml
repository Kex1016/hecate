pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// The dashboard's nav: 32px pine words, the current page on a pine band,
// and the keys on the right.
Item {
    id: root

    property var items: []
    property string current: ""
    // "home": the home page's keys, "settings": a reminder that there is no theme page
    property string hint: "home"
    signal picked(string name)

    implicitHeight: row.implicitHeight

    RowLayout {
        id: row
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: H.HTokens.space8

        Repeater {
            model: root.items
            Rectangle {
                id: link
                required property string modelData
                readonly property bool on: modelData === root.current
                implicitWidth: word.implicitWidth + (on ? 2 * H.HTokens.ch(H.HTokens.fontNav) : 0)
                implicitHeight: word.implicitHeight
                color: on ? H.HTokens.pine : "transparent"

                H.HText {
                    id: word
                    anchors.centerIn: parent
                    text: link.modelData
                    role: "nav"
                    ink: link.on || mouse.containsMouse ? "text" : "pine"
                }
                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.picked(link.modelData)
                }
            }
        }

        Item { Layout.fillWidth: true }

        Row {
            visible: root.hint === "home"
            spacing: H.HTokens.ch(H.HTokens.fontLabel)
            H.Keycap { key: "tab"; hint: "next page ·"; quiet: true }
            H.HText { text: "type to search settings ·"; role: "caption"; ink: "inkMuted" }
            H.Keycap { key: "esc"; hint: "close"; quiet: true }
        }
        H.HText {
            visible: root.hint === "settings"
            text: "no theme page: haiiro is the theme"
            role: "caption"
            ink: "inkMuted"
        }
    }
}
