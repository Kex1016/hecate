pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A battery on a 22x11 pixel grid at 5x: a text outline and terminal, the
// charge in `ink` (foam) over a highlight-med empty part. Always beside the
// percentage, never alone.
//   PixelBattery { level: 0.87 }
PixelGrid {
    id: root

    // 0-1
    property real level: 1
    property string ink: "foam"
    property real cell: 5

    readonly property real unit: px(cell)
    readonly property int steps: 16
    readonly property int charge: Math.max(0, Math.min(steps, Math.round(level * steps)))

    implicitWidth: 22 * unit
    implicitHeight: 11 * unit

    component Part: Rectangle {
        property int gx: 0
        property int gy: 0
        property int gw: 1
        property int gh: 1
        x: gx * root.unit
        y: gy * root.unit
        width: gw * root.unit
        height: gh * root.unit
        color: HTokens.text
    }

    Part { gw: 20 }
    Part { gy: 10; gw: 20 }
    Part { gh: 11 }
    Part { gx: 19; gh: 11 }
    Part { gx: 20; gy: 3; gw: 2; gh: 5 }
    Part { gx: 2; gy: 2; gw: root.charge; gh: 7; color: HTokens.ink(root.ink); visible: root.charge > 0 }
    Part { gx: 2 + root.charge; gy: 2; gw: root.steps - root.charge; gh: 7; color: HTokens.highlightMed; visible: root.charge < root.steps }
}
