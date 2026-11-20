import QtQuick
import QtQuick.Layouts
import qs.services
import qs.modules.common.widgets.haiiro as H

// now playing: the art, title and artist, the position on a Progress track
// and the small transport. Takes whichever player MprisController picked.
DashTile {
    id: root

    headed: false
    interactive: true

    readonly property var player: MprisController.activePlayer
    readonly property bool has: player !== null && (player.trackTitle ?? "") !== ""

    function time(s) {
        s = Math.max(0, Math.floor(s ?? 0));
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0");
    }
    // "org.mpris.MediaPlayer2.spotify" style identities read as one lowercase word
    function playerName(p) {
        return String(p?.identity ?? p?.desktopEntry ?? "").toLowerCase().split(/\s+/)[0];
    }

    Timer {
        interval: 1000
        running: root.player?.isPlaying ?? false
        repeat: true
        onTriggered: root.player.positionChanged()
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        spacing: 14

        Item {
            Layout.alignment: Qt.AlignTop
            implicitWidth: 132
            implicitHeight: 132

            H.Dither {
                anchors.fill: parent
                big: true
                visible: art.status !== Image.Ready
            }
            // Art at half size, doubled with nearest-neighbour like the rest of haiiro's pictures.
            Image {
                id: art
                anchors.fill: parent
                source: root.has ? (root.player.trackArtUrl ?? "") : ""
                sourceSize: Qt.size(66, 66)
                fillMode: Image.PreserveAspectCrop
                smooth: false
                mipmap: false
                asynchronous: true
                visible: status === Image.Ready
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 6

            Row {
                spacing: H.HTokens.space2
                H.HText { text: "now playing"; role: "headingSm" }
                H.HText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.playerName(root.player)
                    role: "caption"
                    ink: "inkMuted"
                }
            }
            H.HText {
                Layout.fillWidth: true
                text: root.has ? root.player.trackTitle : "nothing"
                role: "valueLg"
                elide: Text.ElideRight
            }
            H.HText {
                Layout.fillWidth: true
                text: root.has ? (root.player.trackArtist ?? "") : "The room is quiet."
                ink: root.has ? "foam" : "rose"
                elide: Text.ElideRight
            }
            Item { Layout.fillHeight: true }
            RowLayout {
                Layout.fillWidth: true
                visible: root.has && (root.player.length ?? 0) > 0
                H.HText { text: root.time(root.player?.position); role: "code"; ink: "inkMuted" }
                Item { Layout.fillWidth: true }
                H.HText { text: root.time(root.player?.length); role: "code"; ink: "inkMuted" }
            }
            H.Progress {
                Layout.fillWidth: true
                visible: root.has && (root.player.length ?? 0) > 0
                count: ""
                value: root.has && root.player.length > 0 ? root.player.position / root.player.length : 0
            }
            H.Transport {
                visible: root.player !== null
                small: true
                playing: root.player?.isPlaying ?? false
                canPrevious: root.player?.canGoPrevious ?? false
                canNext: root.player?.canGoNext ?? false
                onPrevious: root.player.previous()
                onPlayPause: root.player.togglePlaying()
                onNext: root.player.next()
            }
        }
    }
}
