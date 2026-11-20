pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import qs.modules.common.widgets.haiiro

// The window a bar segment opens under itself: an HWindow in a popup
// surface, its frame 24px under the bar, centred on the segment but kept
// 24px from the screen's edges. It pixelates in and out, and its surface
// stays mapped until the close frames are done.
Scope {
    id: root

    required property Item target
    property Component content: null
    property string label: ""
    property string variant: "pine"
    property int windowWidth: 300
    property var hints: []
    property bool wanted: false

    readonly property bool hovered: loader.item?.hovered ?? false
    readonly property var popupWindow: loader.item

    LazyLoader {
        id: loader
        active: root.content !== null && (root.wanted || (loader.item?.present ?? false))

        component: PopupWindow {
            id: win

            readonly property int gap: HTokens.space6
            readonly property int edge: HTokens.space6
            readonly property int notch: shift.margin
            readonly property bool hovered: hover.hovered
            readonly property bool present: shift.present

            // where the segment is in the bar's window, and how wide the bar is
            readonly property var bar: root.target.QsWindow.window
            readonly property point origin: root.target.mapToItem(null, 0, 0)
            readonly property real barWidth: bar?.width ?? 1920
            readonly property real barHeight: bar?.height ?? HTokens.barHeight
            readonly property real frameX: Math.max(win.edge, Math.min(win.origin.x + root.target.width / 2 - frame.width / 2, win.barWidth - win.edge - frame.width))

            anchor.item: root.target
            anchor.edges: Edges.Bottom | Edges.Left
            anchor.gravity: Edges.Bottom | Edges.Right
            anchor.adjustment: PopupAdjustment.None
            // a 1px rect on the bar's last row, where the surface's left edge goes
            anchor.rect.x: Math.round(win.frameX - win.notch - win.origin.x)
            anchor.rect.y: Math.round(win.barHeight - 1 - win.origin.y)
            anchor.rect.width: 1
            anchor.rect.height: 1

            implicitWidth: frame.width + 2 * win.notch
            implicitHeight: win.gap + frame.height + win.notch
            color: "transparent"
            visible: true
            // the gap under the bar and the notch's room take no input
            mask: Region {
                item: frame
            }

            PixelTransition {
                id: shift
                x: win.notch
                y: win.gap
                width: frame.width
                height: frame.height
                shown: root.wanted

                HWindow {
                    id: frame
                    width: root.windowWidth
                    label: root.label
                    variant: root.variant
                    hints: root.hints

                    Loader {
                        Layout.fillWidth: true
                        sourceComponent: root.content
                    }
                }

                HoverHandler {
                    id: hover
                }
            }
        }
    }
}
