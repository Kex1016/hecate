pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import qs.modules.common.widgets.haiiro

// Text in the haiiro face. `role` picks a type style from tokens.json
// (body, label, badge, small, headingSm, valueLg, nav, display, code,
// caption); `ink` takes c1-c8 or a token name. `shadow` puts a hard base
// shadow under every glyph, for text that sits straight on the background:
// "free" (2px, shadow-free) or "title" (8px, shadow-title). None by default.
Text {
    id: root

    property string role: "body"
    property string ink: "text"
    property bool bold: false
    property string shadow: "none"
    readonly property var typeStyle: HTokens.role(role)

    color: HTokens.ink(ink)
    renderType: Text.NativeRendering
    textFormat: Text.PlainText
    verticalAlignment: Text.AlignVCenter
    lineHeightMode: Text.FixedHeight
    lineHeight: HTokens.lineBox(typeStyle.size)
    font.family: role === "display" ? HTokens.display : HTokens.mono
    font.bold: bold
    font.pixelSize: typeStyle.size
    font.capitalization: typeStyle.caps ? Font.AllUppercase : Font.MixedCase
    font.letterSpacing: typeStyle.tracking
    font.hintingPreference: Font.PreferFullHinting
    font.kerning: false

    Loader {
        active: root.shadow !== "none"
        z: -1
        sourceComponent: Text {
            // the offset, in whole device pixels
            readonly property real dpr: Math.max(0.25, (QsWindow.window as QsWindow)?.devicePixelRatio ?? 1)
            readonly property real drop: Math.max(1, Math.round((root.shadow === "title" ? HTokens.shadowTitle : HTokens.shadowFree) * dpr)) / dpr
            x: drop
            y: drop
            width: root.width
            height: root.height
            // one ink: colours set in the markup would show through
            text: root.textFormat === Text.PlainText ? root.text : root.text.replace(/\s(color|style)=("[^"]*"|'[^']*')/g, "")
            color: HTokens.base
            linkColor: HTokens.base
            font: root.font
            renderType: root.renderType
            textFormat: root.textFormat
            wrapMode: root.wrapMode
            elide: root.elide
            maximumLineCount: root.maximumLineCount
            horizontalAlignment: root.horizontalAlignment
            verticalAlignment: root.verticalAlignment
            lineHeightMode: root.lineHeightMode
            lineHeight: root.lineHeight
            leftPadding: root.leftPadding
            rightPadding: root.rightPadding
            topPadding: root.topPadding
            bottomPadding: root.bottomPadding
        }
    }
}
