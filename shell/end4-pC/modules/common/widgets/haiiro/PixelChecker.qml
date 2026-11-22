import QtQuick
import qs.modules.common.widgets.haiiro

// A 2x2 checker of one ink over whatever is behind, in whole device pixels:
// the rain's cloud line, or any dithered band of a free widget. For a
// placeholder with a ground colour use Dither. One device pixel high with
// `cell: 3` it is a dashed hairline.
PixelGrid {
    id: root

    property string ink: "subtle"
    property color color: HTokens.ink(ink)
    // one square of the checker, half a dither tile
    property real cell: HTokens.ditherTile / 2

    implicitWidth: 64
    implicitHeight: px(6)

    ShaderEffect {
        readonly property color ink: root.color
        readonly property size size: Qt.size(Math.round(width * root.dpr), Math.round(height * root.dpr))
        readonly property point origin: root.origin
        readonly property real cell: root.dev(root.cell)
        width: Math.round(root.width * root.dpr) / root.dpr
        height: Math.round(root.height * root.dpr) / root.dpr
        fragmentShader: Qt.resolvedUrl("shaders/checker.frag.qsb")
    }
}
