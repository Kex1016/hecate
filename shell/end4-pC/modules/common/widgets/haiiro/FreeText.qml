import QtQuick
import qs.modules.common.widgets.haiiro

// Text of a free widget: HText with the hard base shadow that lets it sit
// straight on the background (shadow-free; shadow-title for the display
// face). `size` sets a pixel size outside the type roles, for the big
// figures free widgets are built around:
//   FreeText { text: "3h 12m"; role: "nav" }                  // .big, 32px
//   FreeText { text: "9°"; role: "display"; size: 120 }
//   FreeText { text: "up since 18:22"; role: "code"; ink: "subtle" }
//   FreeText { text: "├─ cakeos  main"; size: 14; leading: HTokens.leadingPre }
HText {
    id: root

    property int size: 0
    // line height as a share of the size, as the boards set it: the shell's 1.15 for most text,
    // HTokens.leadingPre (1.25) for terminal text that keeps its spacing, HTokens.leadingProse
    // (1.4) for sentences. Display type keeps its own box.
    property real leading: HTokens.lineHeight

    shadow: role === "display" ? "title" : "free"
    font.pixelSize: size > 0 ? size : typeStyle.size
    lineHeight: role === "display" && size > 0 ? size : Math.round(font.pixelSize * leading)
}
