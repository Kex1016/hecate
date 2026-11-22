pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// One square per unit: an hour left, 4% of a disk, a tenth of the day's exp.
// `count` squares, the first `filled` of them in `ink` (text by default;
// gold needs you, foam is a value, love is critical), the rest in
// highlight-med. `large` is the 10px square. Say in a caption what one
// square is.
//   PixelCells { count: 10; filled: 9 }
//   PixelCells { count: 48; filled: 46; ink: "gold"; width: 238 }   // wraps
//   PixelCells { count: 25; filled: 11; large: true }
//   PixelCells { count: 3; lit: step; ink: "rose"; cell: 10; gap: 6 }  // one square lit
// With a width it wraps like text; without one it is a single row.
PixelGrid {
    id: root

    property int count: 10
    property int filled: 0
    // light only this square (stepping squares); -1 uses `filled`
    property int lit: -1
    property string ink: "text"
    property bool large: false
    property real cell: large ? HTokens.pixelCellLarge : HTokens.pixelCell
    property real gap: HTokens.pixelGap
    // squares in a row before it wraps; 0 keeps one row
    property int wrap: 0

    readonly property real side: px(cell)
    readonly property real pitch: side + px(gap)
    // narrower than one row: wrap to what fits. Counted in design pixels, so a
    // row holds the same number of squares at every output scale.
    readonly property int perRow: Math.max(1, wrap > 0 ? wrap : Math.min(count, Math.floor((width + gap + 0.01) / (cell + gap))))
    readonly property int rowCount: Math.max(1, Math.ceil(count / perRow))

    implicitWidth: Math.max(0, Math.min(count, wrap > 0 ? wrap : count) * (cell + gap) - gap)
    implicitHeight: rowCount * pitch - px(gap)

    Repeater {
        model: root.count
        Rectangle {
            required property int index
            x: (index % root.perRow) * root.pitch
            y: Math.floor(index / root.perRow) * root.pitch
            width: root.side
            height: root.side
            color: (root.lit >= 0 ? index === root.lit : index < root.filled) ? HTokens.ink(root.ink) : HTokens.highlightMed
        }
    }
}
