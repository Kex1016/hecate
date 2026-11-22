pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Two series on one baseline: `down` grows up from a 1px subtle line in foam,
// `up` grows down from it in iris, 6px columns 2px apart. It replaces two
// separate sparklines. Each side scales to its own largest value unless
// `downMax` / `upMax` say what fills the half.
//   MirrorSpark { down: [20, 35, 30, 60]; up: [10, 15, 10, 20]; downMax: 100; upMax: 100 }
PixelGrid {
    id: root

    property var down: []
    property var up: []
    property real downMax: 0
    property real upMax: 0
    property string downInk: "foam"
    property string upInk: "iris"

    readonly property real colW: px(6)
    readonly property real pitch: colW + px(HTokens.pixelGap)
    readonly property real half: px(48)
    readonly property real line: px(1)
    readonly property real margin: px(2)
    readonly property int columns: Math.max(down.length, up.length)

    implicitWidth: Math.max(0, columns * pitch - px(HTokens.pixelGap))
    implicitHeight: 2 * half + 2 * margin + line

    function peak(values, max) { return max > 0 ? max : Math.max(1, ...values); }
    // a column's height in whole device pixels; anything above zero shows
    function bar(v, top) {
        const share = Math.max(0, Math.min(1, (Number(v) || 0) / top));
        return share <= 0 ? 0 : Math.max(1, Math.round(share * root.half * root.dpr)) / root.dpr;
    }

    Repeater {
        model: root.down
        Rectangle {
            required property int index
            required property real modelData
            x: index * root.pitch
            width: root.colW
            height: root.bar(modelData, root.peak(root.down, root.downMax))
            y: root.half - height
            color: HTokens.ink(root.downInk)
        }
    }
    Rectangle {
        y: root.half + root.margin
        width: root.width
        height: root.line
        color: HTokens.subtle
    }
    Repeater {
        model: root.up
        Rectangle {
            required property int index
            required property real modelData
            x: index * root.pitch
            y: root.half + 2 * root.margin + root.line
            width: root.colW
            height: root.bar(modelData, root.peak(root.up, root.upMax))
            color: HTokens.ink(root.upInk)
        }
    }
}
