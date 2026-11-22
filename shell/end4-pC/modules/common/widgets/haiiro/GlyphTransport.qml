pragma ComponentBehavior: Bound
import QtQuick
import qs.modules.common.widgets.haiiro

// The free form of Transport: bare pixel glyphs on 44px targets with no
// strip, play/pause larger and pine, each glyph with the 2px base drop
// shadow. `time` ("1:52 / 4:03") sits beside it in subtle. Put a PlayLine
// above it. The first glyph is inset in its target, so pull the row 12px
// left (x: -12) to line the glyph up with the text above.
//   GlyphTransport { playing: true; time: "1:52 / 4:03"; onPlayPause: player.togglePlaying() }
Item {
    id: root

    property bool playing: false
    property bool canPrevious: true
    property bool canNext: true
    property string time: ""
    signal previous()
    signal playPause()
    signal next()

    readonly property int target: HTokens.glyphTarget

    // 8x8 glyphs, one string per row
    readonly property var glyphPrevious: ["", " #    # ", " #   ## ", " #  ### ", " #  ### ", " #   ## ", " #    # ", ""]
    readonly property var glyphNext: ["", " #    # ", " ##   # ", " ###  # ", " ###  # ", " ##   # ", " #    # ", ""]
    readonly property var glyphPause: ["", "  #  #  ", "  #  #  ", "  #  #  ", "  #  #  ", "  #  #  ", "  #  #  ", ""]
    readonly property var glyphPlay: ["", "  #     ", "  ##    ", "  ###   ", "  ###   ", "  ##    ", "  #     ", ""]

    implicitWidth: row.implicitWidth
    implicitHeight: target

    component Button: Item {
        id: button
        property var rows: []
        property bool main: false
        property bool can: true
        signal activated()
        width: root.target
        height: root.target
        activeFocusOnTab: can
        opacity: can ? 1 : 0.55

        Keys.onPressed: event => {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                button.activated();
                event.accepted = true;
            }
        }
        PixelArt {
            id: art
            // 16px glyphs, 24px for play/pause
            cell: button.main ? 3 : 2
            rows: button.rows.map(r => r.padEnd(8, " ").replace(/#/g, button.main ? "P" : mouse.containsMouse && button.can ? "F" : "#"))
            x: Math.round((parent.width - implicitWidth) / 2)
            y: Math.round((parent.height - implicitHeight) / 2)
        }
        Rectangle {
            visible: button.activeFocus
            anchors.fill: parent
            color: "transparent"
            border.width: 2
            border.color: HTokens.foam
        }
        MouseArea {
            id: mouse
            anchors.fill: parent
            enabled: button.can
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.activated()
        }
    }

    Row {
        id: row
        spacing: 0
        Button { rows: root.glyphPrevious; can: root.canPrevious; onActivated: root.previous() }
        Button { rows: root.playing ? root.glyphPause : root.glyphPlay; main: true; onActivated: root.playPause() }
        Button { rows: root.glyphNext; can: root.canNext; onActivated: root.next() }
        Item { width: 8; height: 1; visible: root.time.length > 0 }
        FreeText {
            visible: root.time.length > 0
            anchors.verticalCenter: parent.verticalCenter
            text: root.time
            role: "caption"
            ink: "subtle"
        }
    }
}
