import QtQuick
import qs.modules.common.widgets.haiiro

// The moon on a 24x24 pixel grid at its real phase: the lit part in text,
// the dark side in overlay, and a dithered column where they meet.
//   PixelMoon { illumination: 0.38; waxing: true }
// `illumination` is the lit share of the disc (0 new, 1 full); `waxing`
// puts the light on the right, as it is seen from the north.
PixelArt {
    id: root

    property real illumination: 0
    property bool waxing: true

    cell: 4
    shadow: false
    rows: {
        const f = Math.max(0, Math.min(1, root.illumination));
        // the terminator is half an ellipse: its width goes from the full
        // radius (new) through zero (half) to the other side (full)
        const k = 1 - 2 * f;
        const out = [];
        for (let y = 0; y < 24; y++) {
            const dy = y + 0.5 - 12;
            let row = "";
            for (let x = 0; x < 24; x++) {
                const dx = x + 0.5 - 12;
                const inside = dx * dx + dy * dy;
                if (inside > 115) { row += " "; continue; }
                const side = root.waxing ? dx : -dx;
                const d = side - k * Math.sqrt(Math.max(0, 115 - dy * dy));
                const lit = f >= 0.985 || (f > 0.015 && (d > 0 || (d > -2 && d <= -1 && (x + y) % 2 === 0)));
                row += lit ? "#" : ".";
            }
            out.push(row);
        }
        return out;
    }
}
