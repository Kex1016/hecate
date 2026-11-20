import QtQuick
import qs.modules.common.widgets.haiiro

// A two-tone 2x2 checker for anything without a picture yet (album art,
// avatars, thumbnails). `tone`: "muted" (default) on base, "lo"
// (highlight-med), "pine". `big` doubles the tile to 8px. Set `ink` and
// `ground` directly for anything else; a transparent ground lets what is
// behind show through (that is the Veil).
Item {
    id: root

    property string tone: "muted"
    property bool big: false
    property color ink: tone === "lo" ? HTokens.highlightMed : tone === "pine" ? HTokens.pine : HTokens.muted
    property color ground: HTokens.base
    readonly property int tile: big ? HTokens.ditherTileBig : HTokens.ditherTile

    function hex(c) {
        return "#" + [c.r, c.g, c.b].map(v => Math.round(v * 255).toString(16).padStart(2, "0")).join("");
    }
    function rect(x, y, s, c) {
        return "<rect x='" + x + "' y='" + y + "' width='" + s + "' height='" + s + "' fill='" + hex(c) + "' fill-opacity='" + c.a + "'/>";
    }
    // the conic-gradient checker of bundle.css: ink in the top-right and
    // bottom-left quarters, ground in the others
    readonly property string svg: {
        const t = root.tile, h = t / 2;
        return "<svg xmlns='http://www.w3.org/2000/svg' width='" + t + "' height='" + t + "' shape-rendering='crispEdges'>"
            + (root.ground.a > 0 ? root.rect(0, 0, t, root.ground) : "")
            + root.rect(h, 0, h, root.ink) + root.rect(0, h, h, root.ink) + "</svg>";
    }

    implicitWidth: 64
    implicitHeight: 64

    Image {
        anchors.fill: parent
        source: "data:image/svg+xml;utf8," + encodeURIComponent(root.svg)
        sourceSize: Qt.size(root.tile, root.tile)
        fillMode: Image.Tile
        horizontalAlignment: Image.AlignLeft
        verticalAlignment: Image.AlignTop
        smooth: false
        mipmap: false
        cache: true
        asynchronous: false
    }
}
