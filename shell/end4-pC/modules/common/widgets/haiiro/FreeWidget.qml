pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro

// A widget with no window: type, glyphs and pixel shapes straight on the
// background. Its children stack in a column under the label; `art` puts a
// picture to their left (the design system's `.fw.row`).
//   FreeWidget {
//       label: "uptime"
//       FreeText { text: "3h 12m"; role: "nav" }
//       FreeText { text: "up since 18:22 · load 0.42"; role: "code"; ink: "subtle" }
//   }
//   FreeWidget { label: "spotify"; mark: "♪"; art: FreeArt { ... } ... }
// The label is a prompt line: the command that would print the widget, with
// a rose ». It is also the drag handle (`handle`; null without a label, and
// the widget is then dragged by its body).
// Under the widget the background thins out: the clearing, base at 94% in
// the middle and a 2x2 base checker towards the edge, reaching
// free-clearing-x / free-clearing-y past the content. It is drawn outside
// the widget's bounds and is no part of its size, so snapping and clamping
// use the content box. No frame, no card, no line.
// `live` is for the widget file: false while it is hidden, so timers and
// services can stop. Text inside uses FreeText; shapes come from the kit.
Item {
    id: root

    property string label: ""
    // a glyph before the label, in iris: ♪ for the music widgets
    property string mark: ""
    property bool live: true
    property bool clearing: true
    // Where the clearing is drawn. Left null it sits under this widget only;
    // given an item that lies under every widget, a neighbour's clearing can
    // no longer cover this widget's content.
    property Item clearingLayer: null
    property real spacing: 6
    property real artSpacing: 16
    default property alias content: column.data
    property alias art: artSlot.data
    readonly property Item handle: label.length > 0 ? prompt : null
    readonly property alias column: column

    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight

    // follows the widget around the scene
    PixelGrid {
        id: probe
        anchors.fill: parent
    }

    PixelGrid {
        id: clear

        readonly property real reachX: px(HTokens.freeClearingX)
        readonly property real reachY: px(HTokens.freeClearingY)
        readonly property point at: {
            probe.moved;
            return root.clearingLayer ? root.clearingLayer.mapFromItem(root, 0, 0) : Qt.point(0, 0);
        }

        parent: root.clearingLayer ?? root
        z: root.clearingLayer ? 0 : -1
        visible: root.clearing && root.visible
        x: at.x - reachX
        y: at.y - reachY
        width: Math.round((root.width + 2 * reachX) * dpr) / dpr
        height: Math.round((root.height + 2 * reachY) * dpr) / dpr

        ShaderEffect {
            readonly property color fill: HTokens.clearingFill
            readonly property color base: HTokens.base
            readonly property size size: Qt.size(Math.round(width * clear.dpr), Math.round(height * clear.dpr))
            readonly property point origin: clear.origin
            readonly property real cell: clear.dev(HTokens.ditherTile / 2)
            readonly property real solid: 0.64
            readonly property real edge: 0.84
            width: clear.width
            height: clear.height
            fragmentShader: Qt.resolvedUrl("shaders/clearing.frag.qsb")
        }
    }

    RowLayout {
        id: row
        anchors.fill: parent
        spacing: root.artSpacing

        Item {
            id: artSlot
            visible: children.length > 0
            Layout.alignment: Qt.AlignTop
            implicitWidth: childrenRect.width
            implicitHeight: childrenRect.height
        }

        ColumnLayout {
            id: column
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: true
            spacing: root.spacing

            Row {
                id: prompt
                visible: root.label.length > 0
                spacing: HTokens.ch(HTokens.fontLabel)
                FreeText { text: "»"; role: "caption"; ink: "rose" }
                FreeText { text: root.mark; role: "caption"; ink: "iris"; visible: root.mark.length > 0 }
                FreeText { text: root.label; role: "caption"; ink: "subtle" }
            }
        }
    }
}
