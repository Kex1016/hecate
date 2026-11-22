import QtQuick
import qs.modules.common.widgets.haiiro

// Text of a free widget: HText with the hard base shadow that lets it sit
// straight on the background (shadow-free; shadow-title for the display
// face). `size` sets a pixel size outside the type roles, for the big
// figures free widgets are built around:
//   FreeText { text: "3h 12m"; role: "nav" }                  // .big, 32px
//   FreeText { text: "9°"; role: "display"; size: 120 }
//   FreeText { text: "up since 18:22"; role: "code"; ink: "subtle" }
HText {
    id: root

    property int size: 0

    shadow: role === "display" ? "title" : "free"
    font.pixelSize: size > 0 ? size : typeStyle.size
    lineHeight: size > 0 ? (role === "display" ? size : HTokens.lineBox(size)) : HTokens.lineBox(typeStyle.size)
}
