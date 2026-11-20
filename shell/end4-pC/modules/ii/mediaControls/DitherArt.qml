import QtQuick
import qs.modules.common.widgets.haiiro

// Album art the haiiro way: two tones of the palette (base and muted),
// ordered-dithered at `cells` x `cells` and scaled up with nearest-neighbour,
// so every art pixel is a whole number of screen pixels. While there is no art
// (nothing loaded yet, a failed download, no art url) it shows the Dither
// placeholder instead.
Item {
    id: root

    property url source: ""
    // art resolution: 96 cells drawn at 192px is 2px per cell
    property int cells: 96
    property color dark: HTokens.base
    property color light: HTokens.muted
    readonly property bool ready: canvas.shown

    implicitWidth: 192
    implicitHeight: 192

    // 8x8 Bayer matrix, thresholds 0..63
    readonly property var bayer: [
        0, 32, 8, 40, 2, 34, 10, 42,
        48, 16, 56, 24, 50, 18, 58, 26,
        12, 44, 4, 36, 14, 46, 6, 38,
        60, 28, 52, 20, 62, 30, 54, 22,
        3, 35, 11, 43, 1, 33, 9, 41,
        51, 19, 59, 27, 49, 17, 57, 25,
        15, 47, 7, 39, 13, 45, 5, 37,
        63, 31, 55, 23, 61, 29, 53, 21
    ]

    Dither {
        anchors.fill: parent
        big: true
        visible: !root.ready
    }

    Item {
        anchors.fill: parent
        visible: root.ready
        clip: true

        Canvas {
            id: canvas
            property bool shown: false
            property url loaded: ""
            width: root.cells
            height: root.cells
            scale: root.width / root.cells
            transformOrigin: Item.TopLeft
            smooth: false
            antialiasing: false
            renderTarget: Canvas.Image
            renderStrategy: Canvas.Immediate

            onImageLoaded: requestPaint()
            onPaint: {
                const src = root.source;
                if (src.toString().length === 0 || !isImageLoaded(src)) return;
                const ctx = getContext("2d");
                ctx.reset();
                ctx.drawImage(src, 0, 0, root.cells, root.cells);
                const img = ctx.getImageData(0, 0, root.cells, root.cells);
                const d = img.data;
                const n = root.cells * root.cells;
                const gray = new Array(n);
                let lo = 255, hi = 0;
                for (let i = 0; i < n; i++) {
                    const g = 0.299 * d[4 * i] + 0.587 * d[4 * i + 1] + 0.114 * d[4 * i + 2];
                    gray[i] = g;
                    if (g < lo) lo = g;
                    if (g > hi) hi = g;
                }
                const span = Math.max(1, hi - lo);
                // fillRect, not putImageData: writes to ImageData.data do not stick
                ctx.fillStyle = root.dark;
                ctx.fillRect(0, 0, root.cells, root.cells);
                ctx.fillStyle = root.light;
                for (let y = 0; y < root.cells; y++) {
                    for (let x = 0; x < root.cells; x++) {
                        // stretch to the full range, then compare with the matrix
                        const level = (gray[y * root.cells + x] - lo) / span * 64;
                        if (level > root.bayer[(y & 7) * 8 + (x & 7)] + 0.5)
                            ctx.fillRect(x, y, 1, 1);
                    }
                }
                canvas.loaded = src;
                canvas.shown = true;
            }
        }
    }

    function reload(old) {
        if (old && old.toString().length > 0) canvas.unloadImage(old);
        if (root.source.toString().length === 0) {
            canvas.shown = false;
            return;
        }
        // keep showing the last art until the new one is ready
        canvas.loadImage(root.source);
        if (canvas.isImageLoaded(root.source)) canvas.requestPaint();
    }

    property url previous: ""
    onSourceChanged: {
        const old = previous;
        previous = source;
        reload(old);
    }
    Component.onCompleted: {
        previous = source;
        reload(null);
    }
}
