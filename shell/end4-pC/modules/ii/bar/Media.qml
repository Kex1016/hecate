pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import qs
import qs.services
import qs.services.apple
import qs.modules.common.widgets.haiiro

// Now playing, in iris: "♪ title — artist", at most 260px, cut short when
// the bar runs out. Hover: the track with its position, the visualiser strip
// and the transport, plus "♥ add to library" when Cider plays; click: the
// media panel; middle click: play or pause.
BarSegment {
    id: root

    readonly property MprisPlayer player: MprisController.activePlayer
    readonly property string track: root.player?.trackTitle ?? ""
    readonly property string artist: root.player?.trackArtist ?? ""

    visible: root.track.length > 0
    implicitWidth: Math.min(260, line.implicitWidth)
    popupLabel: "mpris: " + BarState.appName(root.player?.identity ?? root.player?.desktopEntry ?? "player")
    popupWidth: 360
    onClicked: mouse => {
        if (mouse.button === Qt.MiddleButton)
            MprisController.togglePlaying();
        else
            GlobalStates.mediaControlsOpen = !GlobalStates.mediaControlsOpen;
    }
    // the visualiser (cava) runs while the hover shows it
    onPopupOpenChanged: GlobalStates.barMediaHover = root.popupOpen
    Component.onDestruction: if (root.popupOpen) GlobalStates.barMediaHover = false

    HText {
        id: line
        width: root.width
        text: "♪ " + root.track + (root.artist.length > 0 ? " — " + root.artist : "")
        ink: "iris"
        elide: Text.ElideRight
    }

    popup: Component {
        ColumnLayout {
            id: nowPlaying
            spacing: 0

            readonly property MprisPlayer player: root.player
            readonly property real length: nowPlaying.player?.length ?? 0

            // MPRIS only reports the position when asked
            Timer {
                running: nowPlaying.player?.isPlaying ?? false
                interval: 1000
                repeat: true
                triggeredOnStart: true
                onTriggered: nowPlaying.player?.positionChanged()
            }

            HText {
                Layout.fillWidth: true
                text: root.track
                elide: Text.ElideRight
            }
            HText {
                Layout.fillWidth: true
                visible: text.length > 0
                text: root.artist
                ink: "foam"
                elide: Text.ElideRight
            }
            HText {
                Layout.fillWidth: true
                visible: text.length > 0
                text: nowPlaying.player?.trackAlbum ?? ""
                ink: "muted"
                role: "small"
                elide: Text.ElideRight
            }
            Progress {
                Layout.fillWidth: true
                Layout.topMargin: HTokens.space3
                visible: nowPlaying.length > 0
                label: nowPlaying.player?.isPlaying ? "playing" : "paused"
                count: BarState.clock(nowPlaying.player?.position ?? 0) + " / " + BarState.clock(nowPlaying.length)
                value: nowPlaying.length > 0 ? (nowPlaying.player?.position ?? 0) / nowPlaying.length : 0
            }
            Spark {
                Layout.fillWidth: true
                Layout.topMargin: HTokens.space3
                visible: (nowPlaying.player?.isPlaying ?? false) && GlobalStates.visualizerPoints.length > 0
                values: GlobalStates.visualizerPoints.slice(0, Math.floor(width / 8))
                ink: "iris"
                max: 1000
            }
            Transport {
                Layout.topMargin: HTokens.space3
                small: true
                playing: nowPlaying.player?.isPlaying ?? false
                canPrevious: MprisController.canGoPrevious
                canNext: MprisController.canGoNext
                onPrevious: MprisController.previous()
                onPlayPause: MprisController.togglePlaying()
                onNext: MprisController.next()
            }
            RowLayout {
                Layout.topMargin: HTokens.space3
                visible: Cider.active && Cider.track !== null
                spacing: HTokens.space2
                InkButton {
                    text: Cider.inLibrary ? "♥ in library" : "♥ add to library"
                    ink: Cider.inLibrary ? "c1" : "c3"
                    enabled: !Cider.inLibrary
                    onClicked: Cider.addToLibrary()
                }
                HText {
                    text: "via cider"
                    ink: "muted"
                    role: "caption"
                }
            }
        }
    }
}
