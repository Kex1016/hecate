import QtQuick
import qs.modules.common.widgets.haiiro

// The sun's path on a 60x18 pixel grid: the horizon as a subtle line, the
// path as a dotted arc, and a gold dot where the sun is now. At night the
// arc hangs under the horizon (sunset on the left, sunrise on the right);
// by day it stands on it.
//   SunPath { progress: 0.37; up: false }   // 37% of the way to sunrise
PixelArt {
    id: root

    // 0-1 along the arc, left to right
    property real progress: 0
    // the sun is above the horizon
    property bool up: false

    cell: 4
    shadow: false
    rows: {
        const grid = [];
        for (let y = 0; y < 18; y++) grid.push(new Array(60).fill(" "));
        const put = (x, y, ch) => {
            const gy = root.up ? 17 - y : y;
            if (gy >= 0 && gy < 18 && x >= 0 && x < 60) grid[gy][x] = ch;
        };
        for (let x = 0; x < 60; x++) put(x, 2, "+");
        const at = t => Qt.point(Math.round(2 + 55 * t), Math.round(3 + 12 * Math.sin(Math.PI * t)));
        for (let i = 0; i <= 20; i++) {
            const p = at(i / 20);
            put(p.x, p.y, "+");
        }
        const sun = at(Math.max(0, Math.min(1, root.progress)));
        for (let dy = -1; dy <= 1; dy++)
            for (let dx = -1; dx <= 1; dx++) put(sun.x + dx, sun.y + dy, "G");
        return grid.map(r => r.join(""));
    }
}
