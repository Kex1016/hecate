import QtQuick
import qs.modules.common.widgets.haiiro as H

// The haiiro mark at 64px: 灰色 in pixel kanji, pine on base inside a pine
// frame. The VGA face has no kanji, so fontconfig draws them from Unifont on
// the same 16px grid. A stand-in until the logo image is in the shell's assets.
Rectangle {
    implicitWidth: 64
    implicitHeight: 64
    color: H.HTokens.base
    border.width: 2
    border.color: H.HTokens.pine

    H.HText {
        anchors.centerIn: parent
        text: "灰色"
        font.pixelSize: 16
        lineHeight: 16
        ink: "pine"
    }
}
