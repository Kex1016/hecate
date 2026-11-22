pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// A ring of 60 squares around a value: the squares used up in highlight-med,
// the rest in `ink` (gold for the timer), clockwise from the top centre.
// What it counts goes in the middle, as children.
//   PixelRing { done: 15
//       FreeText { anchors.centerIn: parent; text: "18:42"; role: "nav"; ink: "gold" } }
PixelGrid {
    id: root

    // squares used up, 0-60
    property int done: 0
    property string ink: "gold"

    readonly property int total: 60
    readonly property int perSide: 16
    readonly property real side: px(HTokens.pixelCell)
    readonly property real pitch: side + px(HTokens.pixelGap)
    readonly property real inset: px(1)

    implicitWidth: perSide * pitch - px(HTokens.pixelGap) + 2 * inset
    implicitHeight: implicitWidth

    // square i of 60 on the 16x16 frame, starting right of the top centre
    function cellAt(i) {
        const last = root.perSide - 1;
        const k = (i + 8) % root.total;
        if (k <= last) return Qt.point(k, 0);
        if (k <= 2 * last) return Qt.point(last, k - last);
        if (k <= 3 * last) return Qt.point(3 * last - k, last);
        return Qt.point(0, 4 * last - k);
    }

    Repeater {
        model: root.total
        Rectangle {
            required property int index
            readonly property point at: root.cellAt(index)
            x: root.inset + at.x * root.pitch
            y: root.inset + at.y * root.pitch
            width: root.side
            height: root.side
            color: index < root.done ? HTokens.highlightMed : HTokens.ink(root.ink)
        }
    }
}
