import QtQuick
import qs.modules.common.widgets.haiiro as H

// A page title: one lowercase word in Jacquard 24 with the hard 8px
// base-coloured shadow (shadow-title). The canvas sets it at 120px.
Item {
    id: root

    property string text: ""
    property int size: 120

    implicitWidth: face.implicitWidth + 8
    implicitHeight: face.implicitHeight + 8

    H.HText {
        x: 8
        y: 8
        text: root.text
        role: "display"
        font.pixelSize: root.size
        lineHeight: root.size
        ink: "base"
    }
    H.HText {
        id: face
        text: root.text
        role: "display"
        font.pixelSize: root.size
        lineHeight: root.size
    }
}
