import QtQuick
import qs.modules.common.widgets.haiiro

// Text in the haiiro face. `role` picks a type style from tokens.json
// (body, label, badge, small, headingSm, valueLg, nav, display, code,
// caption); `ink` takes c1-c8 or a token name.
Text {
    id: root

    property string role: "body"
    property string ink: "text"
    property bool bold: false
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
}
