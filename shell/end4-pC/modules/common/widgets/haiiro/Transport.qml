pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// Media controls: one hairline strip of equal cells (previous, play/pause,
// next) with play/pause wider and filled pine. The glyphs are pixel shapes on
// an 8x8 grid, the one place the shell draws icons. `small` is the 36x28
// strip for the bar hover, dashboard tile and lock screen. Shuffle and repeat
// are Chips beside the strip, never inside it.
Item {
    id: root

    property bool playing: false
    property bool small: false
    property bool canPrevious: true
    property bool canNext: true
    signal previous()
    signal playPause()
    signal next()

    readonly property int cellW: small ? HTokens.transportCellSmall : HTokens.transportCell
    readonly property int mainW: small ? HTokens.transportMainSmall : HTokens.transportMain
    readonly property int cellH: small ? 28 : 40
    readonly property int line: HTokens.borderHairline

    // 8x8 glyphs as [x, y, w, h] rects
    readonly property var glyphPrevious: [[1, 0, 1, 8], [3, 3, 1, 2], [4, 2, 1, 4], [5, 1, 1, 6], [6, 0, 1, 8]]
    readonly property var glyphNext: [[1, 0, 1, 8], [2, 1, 1, 6], [3, 2, 1, 4], [4, 3, 1, 2], [6, 0, 1, 8]]
    readonly property var glyphPause: [[1, 0, 2, 8], [5, 0, 2, 8]]
    readonly property var glyphPlay: [[2, 0, 1, 8], [3, 1, 1, 6], [4, 2, 1, 4], [5, 3, 1, 2]]

    implicitWidth: 2 * cellW + mainW + 2 * line + 2 * line
    implicitHeight: cellH + 2 * line

    component Glyph: Item {
        id: glyph
        property var rects: []
        property color color: HTokens.text
        width: 16
        height: 16
        Repeater {
            model: glyph.rects
            Rectangle {
                required property var modelData
                x: modelData[0] * 2
                y: modelData[1] * 2
                width: modelData[2] * 2
                height: modelData[3] * 2
                color: glyph.color
            }
        }
    }

    component Cell: Item {
        id: cell
        property var rects: []
        property bool main: false
        property bool can: true
        signal activated()
        width: main ? root.mainW : root.cellW
        height: root.cellH
        activeFocusOnTab: can
        opacity: can ? 1 : 0.55

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                cell.activated();
                event.accepted = true;
            }
        }
        Rectangle {
            anchors.fill: parent
            color: cell.main ? HTokens.pine : mouse.containsMouse && cell.can ? HTokens.overlay : "transparent"
        }
        Glyph {
            anchors.centerIn: parent
            rects: cell.rects
        }
        Rectangle {
            visible: cell.activeFocus
            anchors.fill: parent
            anchors.margins: -2
            z: 1
            color: "transparent"
            border.width: 2
            border.color: HTokens.foam
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            enabled: cell.can
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: cell.activated()
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.width: root.line
        border.color: HTokens.inkHairline
    }

    Row {
        x: root.line
        y: root.line
        spacing: 0

        Cell {
            rects: root.glyphPrevious
            can: root.canPrevious
            onActivated: root.previous()
        }
        Rectangle { width: root.line; height: root.cellH; color: HTokens.inkHairline }
        Cell {
            main: true
            rects: root.playing ? root.glyphPause : root.glyphPlay
            onActivated: root.playPause()
        }
        Rectangle { width: root.line; height: root.cellH; color: HTokens.inkHairline }
        Cell {
            rects: root.glyphNext
            can: root.canNext
            onActivated: root.next()
        }
    }
}
