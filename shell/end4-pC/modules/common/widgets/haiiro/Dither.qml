import QtQuick
import qs.modules.common.widgets.haiiro

// A two-tone 2x2 checker for anything without a picture yet (album art,
// avatars, thumbnails). `tone`: "muted" (default) on base, "lo"
// (highlight-med), "pine". `big` doubles the tile to 8px. Set `ink` and
// `ground` directly for anything else; a transparent ground lets what is
// behind show through (that is the Veil).
// Drawn by a shader on whole device pixels, so it stays sharp at 1.25 and
// 1.75; Qt's software renderer gets the two inks blended instead.
PixelGrid {
    id: root

    property string tone: "muted"
    property bool big: false
    property color ink: tone === "lo" ? HTokens.highlightMed : tone === "pine" ? HTokens.pine : HTokens.muted
    property color ground: HTokens.base
    readonly property int tile: big ? HTokens.ditherTileBig : HTokens.ditherTile
    readonly property bool shaded: GraphicsInfo.api !== GraphicsInfo.Software

    implicitWidth: 64
    implicitHeight: 64

    // the conic-gradient checker of bundle.css: ink in the top-right and
    // bottom-left quarters, ground in the others
    ShaderEffect {
        visible: root.shaded
        readonly property color ink: root.ink
        readonly property color ground: root.ground
        readonly property size size: Qt.size(Math.round(width * root.dpr), Math.round(height * root.dpr))
        readonly property real cell: root.dev(root.tile / 2)
        width: Math.round(root.width * root.dpr) / root.dpr
        height: Math.round(root.height * root.dpr) / root.dpr
        fragmentShader: Qt.resolvedUrl("shaders/dither.frag.qsb")
    }

    Rectangle {
        visible: !root.shaded
        width: root.width
        height: root.height
        color: {
            const i = root.ink, g = root.ground, a = (i.a + g.a) / 2;
            const mix = (x, y) => a > 0 ? (x * i.a + y * g.a) / 2 / a : 0;
            return Qt.rgba(mix(i.r, g.r), mix(i.g, g.g), mix(i.b, g.b), a);
        }
    }
}
