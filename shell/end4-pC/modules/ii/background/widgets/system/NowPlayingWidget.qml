pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import qs.services
import qs.modules.common.widgets.haiiro as H

// now playing · the player MprisController picked (Spotify or Cider first), its cover,
// title, artist and how far in. A click plays or pauses.
H.HWindow {
    id: root

    property bool live: true
    readonly property MprisPlayer player: MprisController.activePlayer
    readonly property string playerName: root.player ? String(root.player.identity).toLowerCase() : ""
    readonly property bool hasTrack: !!root.player && (root.player.trackTitle ?? "") !== ""
    readonly property real progress: root.player && root.player.length > 0
        ? root.player.position / root.player.length : 0

    label: root.player ? `♪ ${root.playerName}` : "♪ media"
    labelInk: "c7"
    widget: true
    implicitWidth: 360

    // MPRIS only reports the position on seeks; ask for it while playing.
    Timer {
        running: root.live && (root.player?.isPlaying ?? false)
        interval: 1000
        repeat: true
        onTriggered: root.player.positionChanged()
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: H.HTokens.space3
        visible: root.hasTrack

        Item {
            Layout.preferredWidth: 72
            Layout.preferredHeight: 72

            H.Dither {
                anchors.fill: parent
                big: true
                visible: cover.status !== Image.Ready
            }
            Image {
                id: cover
                anchors.fill: parent
                source: root.player ? root.player.trackArtUrl : ""
                sourceSize: Qt.size(72, 72)
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
            spacing: H.HTokens.space1

            H.HText {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: root.player?.trackTitle ?? ""
            }
            H.HText {
                Layout.fillWidth: true
                role: "code"
                ink: "c6"
                elide: Text.ElideRight
                text: root.player?.trackArtist ?? ""
            }
            Item { Layout.fillHeight: true }
            H.Progress {
                Layout.fillWidth: true
                value: root.progress
            }
        }
    }

    H.HText {
        visible: !root.hasTrack
        ink: "c1"
        text: "nothing playing"
    }

    TapHandler {
        enabled: root.player?.canTogglePlaying ?? false
        onTapped: MprisController.togglePlaying()
    }
}
