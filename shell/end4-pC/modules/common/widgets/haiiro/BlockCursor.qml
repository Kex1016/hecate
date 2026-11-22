import QtQuick
import qs.modules.common.widgets.haiiro

// The block cursor of a free widget: 1ch by one glyph (8x16 at the body
// size), in `ink`. It does not blink. `size` follows the text it ends.
//   Row { FreeText { text: "The tea has gone cold."; ink: "rose"; role: "nav" }
//         BlockCursor { ink: "rose"; size: 32 } }
PixelGrid {
    id: root

    property string ink: "text"
    property int size: HTokens.fontBody

    implicitWidth: px(HTokens.ch(size))
    implicitHeight: px(size)

    Rectangle {
        width: root.implicitWidth
        height: root.implicitHeight
        color: HTokens.ink(root.ink)
    }
}
