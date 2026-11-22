pragma ComponentBehavior: Bound
import QtQuick
import Quickshell

// The base of every pixel shape: it keeps what is inside on whole device
// pixels at any output scale. At 1.25 an 8px cell is 10 device pixels and a
// 2px gap is 2.5, which would blur or come out uneven; `px()` rounds a
// length to whole device pixels, and the content is nudged so its origin
// lands on one too, wherever the layout put this item.
//   PixelGrid {
//       implicitWidth: 3 * px(8) + 2 * px(2)
//       Rectangle { width: parent.px(8); height: width }
//   }
// Sizes are implicit; a layout may place the item anywhere.
Item {
    id: root

    default property alias content: grid.data
    readonly property QsWindow win: QsWindow.window as QsWindow
    readonly property real dpr: Math.max(0.25, win?.devicePixelRatio ?? Screen.devicePixelRatio ?? 1)
    // the nudge that puts the content's origin on a device pixel
    property real snapX: 0
    property real snapY: 0
    // the content's origin in the scene, in device pixels
    property point origin: Qt.point(0, 0)
    // a counter that changes whenever the item moved or the scale changed
    property int moved: 0

    // `n` logical pixels as a whole number of device pixels, never less than one
    function dev(n) { return n === 0 ? 0 : Math.max(1, Math.round(Math.abs(n) * dpr)) * (n < 0 ? -1 : 1) }
    // the same length back in logical pixels, for x, y, width and height
    function px(n) { return dev(n) / dpr }

    property var watched: []
    function resnap() {
        const p = root.mapToItem(null, 0, 0);
        const dx = Math.round(p.x * dpr), dy = Math.round(p.y * dpr);
        const sx = dx / dpr - p.x, sy = dy / dpr - p.y;
        if (Math.abs(sx - snapX) > 1e-4) snapX = sx;
        if (Math.abs(sy - snapY) > 1e-4) snapY = sy;
        if (dx !== origin.x || dy !== origin.y) origin = Qt.point(dx, dy);
        moved++;
    }
    function unwatch() {
        for (const item of watched) {
            item.xChanged.disconnect(root.resnap);
            item.yChanged.disconnect(root.resnap);
            item.parentChanged.disconnect(root.rewatch);
        }
        watched = [];
    }
    // an item does not hear its ancestors move, so listen to each of them
    function rewatch() {
        unwatch();
        const list = [];
        for (let item = root; item; item = item.parent) {
            item.xChanged.connect(root.resnap);
            item.yChanged.connect(root.resnap);
            item.parentChanged.connect(root.rewatch);
            list.push(item);
        }
        watched = list;
        resnap();
    }

    onWinChanged: rewatch()
    onDprChanged: resnap()
    onVisibleChanged: if (visible) resnap()
    Component.onCompleted: rewatch()
    Component.onDestruction: unwatch()

    Item {
        id: grid
        x: root.snapX
        y: root.snapY
        width: root.width
        height: root.height
    }
}
