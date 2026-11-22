import QtQuick
import qs.modules.common.widgets.haiiro

// Album art or a portrait in a free widget: the picture floats on a hard
// base shadow (shadow-art, 8px; `offset: 4` for the small art of a list).
// Put the picture inside; it fills the frame.
//   FreeArt { width: 112; height: 112
//       Dither { anchors.fill: parent; big: true } }
PixelGrid {
    id: root

    default property alias picture: frame.data
    property real offset: HTokens.shadowArt

    implicitWidth: 96
    implicitHeight: 96

    Rectangle {
        x: root.px(root.offset)
        y: root.px(root.offset)
        width: frame.width
        height: frame.height
        color: HTokens.base
        visible: root.offset > 0
    }
    Item {
        id: frame
        width: Math.round(root.width * root.dpr) / root.dpr
        height: Math.round(root.height * root.dpr) / root.dpr
        clip: true
    }
}
