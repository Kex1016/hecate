pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A small picture drawn on a pixel grid, the way the design system draws its
// SVGs with crispEdges. `rows` is the picture, one string per row and one
// character per art pixel:
//   #  text          +  subtle       h  highlight-high   o  highlight-med
//   .  overlay       m  muted        F  foam             P  pine
//   G  gold          L  love         R  rose             I  iris
//   B  base          (space: nothing)
// Each art pixel is `cell` logical pixels, snapped to whole device pixels.
// `shadow` puts the 2px base drop shadow under it (shadow-free). `dim` swaps
// in the dim set (highlight-high and highlight-med) for a part that is away.
// `mirror` flips it left to right. Runs of one ink are merged into single
// rectangles, so a 24x24 moon is a few dozen items, not 576.
PixelGrid {
    id: root

    property var rows: []
    property real cell: 4
    property bool shadow: true
    property bool dim: false
    property bool mirror: false

    readonly property int cols: rows.reduce((n, r) => Math.max(n, r.length), 0)
    readonly property real unit: px(cell)
    readonly property real drop: shadow ? px(HTokens.shadowFree) : 0

    implicitWidth: cols * unit
    implicitHeight: rows.length * unit

    function ink(ch) {
        if (root.dim) {
            switch (ch) {
            case ".": return HTokens.overlay
            case "#": return HTokens.highlightHigh
            default: return HTokens.highlightMed
            }
        }
        switch (ch) {
        case "#": return HTokens.text
        case "+": return HTokens.subtle
        case "h": return HTokens.highlightHigh
        case "o": return HTokens.highlightMed
        case ".": return HTokens.overlay
        case "m": return HTokens.muted
        case "F": return HTokens.foam
        case "P": return HTokens.pine
        case "G": return HTokens.gold
        case "L": return HTokens.love
        case "R": return HTokens.rose
        case "I": return HTokens.iris
        default: return HTokens.base
        }
    }

    // [x, y, w, h, ch] for every run; equal runs on consecutive rows join.
    // With `solid`, every painted pixel counts as the same ink (the shadow).
    function runs(solid) {
        const out = [];
        let open = {};
        for (let y = 0; y < rows.length; y++) {
            const src = rows[y];
            const row = root.mirror ? src.padEnd(root.cols, " ").split("").reverse().join("") : src;
            const next = {};
            let x = 0;
            while (x < row.length) {
                const ch = row[x] === " " ? " " : solid ? "B" : row[x];
                let end = x + 1;
                while (end < row.length && (row[end] === " " ? " " : solid ? "B" : row[end]) === ch) end++;
                if (ch !== " ") {
                    const key = x + ":" + end + ":" + ch;
                    const above = open[key];
                    if (above) { above[3]++; next[key] = above; }
                    else { const r = [x, y, end - x, 1, ch]; out.push(r); next[key] = r; }
                }
                x = end;
            }
            open = next;
        }
        return out;
    }

    Repeater {
        model: root.shadow ? root.runs(true) : []
        Rectangle {
            required property var modelData
            x: modelData[0] * root.unit + root.drop
            y: modelData[1] * root.unit + root.drop
            width: modelData[2] * root.unit
            height: modelData[3] * root.unit
            color: HTokens.base
        }
    }
    Repeater {
        model: root.runs(false)
        Rectangle {
            required property var modelData
            x: modelData[0] * root.unit
            y: modelData[1] * root.unit
            width: modelData[2] * root.unit
            height: modelData[3] * root.unit
            color: root.ink(modelData[4])
        }
    }
}
