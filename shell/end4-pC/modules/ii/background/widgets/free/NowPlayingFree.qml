pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import qs.services
import qs.modules.common.widgets.haiiro as H

// now playing, free · the cover floats on its hard shadow, the title in the nav size, the
// artist in foam, a dotted playhead and the bare glyph transport. The player is the one
// MprisController picked (Spotify or Cider first), as in the window.
H.FreeWidget {
    id: root

    readonly property MprisPlayer player: MprisController.activePlayer
    readonly property string playerName: root.player ? String(root.player.identity).toLowerCase() : ""
    readonly property bool hasTrack: !!root.player && (root.player.trackTitle ?? "") !== ""
    readonly property real length: root.player?.length ?? 0
    readonly property real position: root.player?.position ?? 0
    // the text column; with the 112px cover and the gap that is the window's 360px
    readonly property real columnWidth: 232

    label: root.player ? root.playerName : "media"
    mark: "♪"
    // the board stacks the text 4px apart
    spacing: 4

    // 28 x 28 squares of 4px, ink where x + y is odd
    readonly property var checker: Array.from({ length: 28 }, (_, y) => Array.from({ length: 28 }, (_, x) => (x + y) % 2 === 1 ? "m" : " ").join(""))

    function clock(seconds) {
        const s = Math.max(0, Math.floor(seconds || 0));
        return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
    }

    // MPRIS only reports the position on seeks; ask for it while playing.
    Timer {
        running: root.live && (root.player?.isPlaying ?? false)
        interval: 1000
        repeat: true
        onTriggered: root.player.positionChanged()
    }

    // shown with nothing playing too, as the placeholder, so the widget keeps its shape
    art: H.FreeArt {
        width: 112
        height: 112

        // the placeholder until a cover is there: the big dither, a 4px checker of muted on
        // base. Drawn here and not with PixelChecker, whose grid is the scene's, so the top
        // left square is always base and the art looks the same wherever it sits.
        Rectangle {
            anchors.fill: parent
            color: H.HTokens.base
        }
        H.PixelArt {
            visible: cover.status !== Image.Ready
            cell: H.HTokens.ditherTileBig / 2
            shadow: false
            rows: root.checker
        }
        Image {
            id: cover
            anchors.fill: parent
            source: root.player ? root.player.trackArtUrl : ""
            sourceSize: Qt.size(112, 112)
            fillMode: Image.PreserveAspectCrop
            smooth: false
            mipmap: false
            asynchronous: true
            visible: status === Image.Ready
        }
    }

    H.FreeText {
        visible: root.hasTrack
        Layout.preferredWidth: root.columnWidth
        role: "nav"
        elide: Text.ElideRight
        text: root.player?.trackTitle ?? ""
    }
    H.FreeText {
        visible: root.hasTrack
        Layout.preferredWidth: root.columnWidth
        role: "code"
        ink: "c6"
        elide: Text.ElideRight
        text: root.player?.trackArtist ?? ""
    }
    H.PlayLine {
        visible: root.hasTrack
        Layout.preferredWidth: root.columnWidth
        Layout.topMargin: 6
        progress: root.length > 0 ? root.position / root.length : 0
        seekable: (root.player?.canSeek ?? false) && root.length > 0
        onSeek: p => root.player.position = p * root.length
    }
    Item {
        visible: root.hasTrack
        Layout.preferredWidth: root.columnWidth
        implicitHeight: transport.implicitHeight

        H.GlyphTransport {
            id: transport
            x: -12
            playing: !!MprisController.isPlaying
            canPrevious: MprisController.canGoPrevious
            canNext: MprisController.canGoNext
            time: `${root.clock(root.position)} / ${root.clock(root.length)}`
            onPrevious: MprisController.previous()
            onPlayPause: MprisController.togglePlaying()
            onNext: MprisController.next()
        }
    }

    H.FreeText {
        visible: !root.hasTrack
        ink: "c1"
        text: "nothing playing"
    }
}
