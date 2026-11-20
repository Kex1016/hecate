pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import qs.modules.common.widgets.haiiro
import qs.services
import qs.services.apple
import "playerPriority.js" as Priority

// The media panel (Mod+M): the haiiro window `hecate: ~/media` with the
// dithered art, the track, the seek bar, the transport strip with the shuffle
// and repeat chips, and one chip per open player.
//
// `player` is the one shown (an MprisPlayer, or anything with the same
// properties); `players` is every open player, best first. Picking a chip
// emits playerPicked, and MediaControls decides what that means.
HWindow {
    id: root

    property var player: null
    property var players: []
    signal playerPicked(var player)
    signal equalizerRequested()
    signal closeRequested()

    readonly property bool hasPlayer: player !== null && player !== undefined
    readonly property real length: hasPlayer && player.lengthSupported ? Math.max(0, player.length) : 0
    readonly property real position: hasPlayer && player.positionSupported ? Math.max(0, Math.min(player.position, length > 0 ? length : player.position)) : 0
    readonly property bool canSeek: hasPlayer && length > 0 && player.canSeek
    readonly property string title: hasPlayer ? (player.trackTitle || "unknown title") : ""
    readonly property string artist: hasPlayer ? (player.trackArtist || "unknown artist") : ""
    readonly property string album: hasPlayer ? (player.trackAlbum || "") : ""

    function clock(seconds) {
        const s = Math.max(0, Math.floor(seconds));
        const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
        const two = n => (n < 10 ? "0" : "") + n;
        return h > 0 ? h + ":" + two(m) + ":" + two(r) : m + ":" + two(r);
    }

    function seekBy(seconds) {
        if (!root.canSeek) return;
        root.player.position = Math.max(0, Math.min(root.length, root.position + seconds));
    }

    function nextPlayer() {
        if (root.players.length < 2) return;
        const i = root.players.indexOf(root.player);
        root.playerPicked(root.players[(i + 1) % root.players.length]);
    }

    // none -> playlist -> track -> none
    function cycleLoop() {
        if (!root.hasPlayer || !root.player.loopSupported) return;
        const s = root.player.loopState;
        root.player.loopState = s === MprisLoopState.None ? MprisLoopState.Playlist : s === MprisLoopState.Playlist ? MprisLoopState.Track : MprisLoopState.None;
    }

    label: "hecate: ~/media"
    hints: root.hasPlayer ? [
        { key: "space", text: "play/pause" },
        { key: "←→", text: "seek" },
        { key: "tab", text: "player" },
        { key: "e", text: "equalizer" }
    ] : [{ key: "esc", text: "close" }]
    implicitWidth: HTokens.panelWidth

    // MPRIS does not push the position, so ask for it while something plays.
    Timer {
        running: root.hasPlayer && root.player.isPlaying
        interval: 1000
        repeat: true
        onTriggered: root.player.positionChanged()
    }

    // Tab and the arrows are claimed by Qt's focus chain and by the seek bar
    // before a Keys handler sees them, so they are shortcuts
    Shortcut { sequence: "Tab"; enabled: root.hasPlayer; onActivated: root.nextPlayer() }
    Shortcut { sequence: "Left"; enabled: root.hasPlayer; onActivated: root.seekBy(-5) }
    Shortcut { sequence: "Right"; enabled: root.hasPlayer; onActivated: root.seekBy(5) }

    // the other keys, whichever child has focus
    Keys.onPressed: event => {
        if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return;
        if (event.key === Qt.Key_Escape) root.closeRequested();
        else if (!root.hasPlayer) return;
        else if (event.key === Qt.Key_Space) { if (root.player.canTogglePlaying) root.player.togglePlaying(); }
        else if (event.key === Qt.Key_E) root.equalizerRequested();
        else return;
        event.accepted = true;
    }
    focus: true

    // ---- nothing open ----
    HText {
        visible: !root.hasPlayer
        Layout.fillWidth: true
        text: "Nothing is playing."
        role: "nav"
    }
    HText {
        visible: !root.hasPlayer
        Layout.fillWidth: true
        Layout.topMargin: 8
        text: "Open spotify or cider and it shows up here; a browser or mpv does too, when neither of those is open."
        ink: "c1"
        wrapMode: Text.WordWrap
    }

    // ---- art and track ----
    RowLayout {
        visible: root.hasPlayer
        Layout.fillWidth: true
        spacing: 16

        DitherArt {
            Layout.preferredWidth: 192
            Layout.preferredHeight: 192
            Layout.alignment: Qt.AlignTop
            source: root.hasPlayer ? (root.player.trackArtUrl ?? "") : ""
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            spacing: 6

            HLabel {
                text: root.hasPlayer ? Priority.shortName(root.player) : ""
            }
            HText {
                Layout.fillWidth: true
                text: root.title
                role: "nav"
                elide: Text.ElideRight
                maximumLineCount: 1
            }
            HText {
                Layout.fillWidth: true
                text: root.artist
                ink: "c6"
                elide: Text.ElideRight
            }
            HText {
                Layout.fillWidth: true
                visible: root.album.length > 0
                text: root.album
                ink: "c1"
                elide: Text.ElideRight
            }
            Item { Layout.fillHeight: true }

            RowLayout {
                Layout.fillWidth: true
                HText { text: root.length > 0 ? root.clock(root.position) : "-:--"; role: "caption"; ink: "inkMuted" }
                Item { Layout.fillWidth: true }
                HText { text: root.length > 0 ? root.clock(root.length) : "-:--"; role: "caption"; ink: "inkMuted" }
            }
            Slider {
                id: seek
                Layout.fillWidth: true
                showValue: false
                thumb: root.canSeek
                enabled: root.canSeek
                from: 0
                to: Math.max(1, root.length)
                stepSize: 1
                onMoved: value => { if (root.canSeek) root.player.position = value; }
                // setValue() replaces the value, so a plain binding would stop
                // following the player after the first drag
                Binding {
                    target: seek
                    property: "value"
                    value: root.position
                    when: !seek.pressed
                }
            }
        }
    }

    // ---- shuffle, transport, repeat ----
    RowLayout {
        visible: root.hasPlayer
        Layout.fillWidth: true
        Layout.topMargin: 14

        Chip {
            text: "shuffle"
            interactive: true
            enabled: root.hasPlayer && root.player.shuffleSupported && root.player.canControl
            on: root.hasPlayer && root.player.shuffle
            onClicked: root.player.shuffle = !root.player.shuffle
        }
        Item { Layout.fillWidth: true }
        Transport {
            playing: root.hasPlayer && root.player.isPlaying
            canPrevious: root.hasPlayer && root.player.canGoPrevious
            canNext: root.hasPlayer && root.player.canGoNext
            onPrevious: root.player.previous()
            onPlayPause: { if (root.player.canTogglePlaying) root.player.togglePlaying(); }
            onNext: root.player.next()
        }
        Item { Layout.fillWidth: true }
        Chip {
            text: root.hasPlayer && root.player.loopState === MprisLoopState.Track ? "repeat one" : "repeat"
            interactive: true
            enabled: root.hasPlayer && root.player.loopSupported && root.player.canControl
            on: root.hasPlayer && root.player.loopState !== MprisLoopState.None
            onClicked: root.cycleLoop()
        }
    }

    // ---- lyrics: Cider's synced lines while it plays, else the lyrics service's ----
    // Cider follows the line closely only while someone shows it.
    property bool ciderHeld: false
    readonly property bool wantsCider: root.visible && Cider.open
    onWantsCiderChanged: {
        if (root.wantsCider && !root.ciderHeld) { Cider.consumers++; root.ciderHeld = true; }
        else if (!root.wantsCider && root.ciderHeld) { Cider.consumers--; root.ciderHeld = false; }
    }
    Component.onDestruction: if (root.ciderHeld) Cider.consumers--
    ColumnLayout {
        id: lyrics
        readonly property bool fromCider: Cider.active && Cider.hasLines
        readonly property bool fromService: !fromCider && LyricsService.synced
            && LyricsService.activePlayer === root.player
        readonly property var lines: fromCider ? [Cider.previousLine, Cider.currentLine, Cider.nextLine]
            : fromService ? LyricsService.slots.slice(LyricsService.before - 1, LyricsService.before + 2) : []
        visible: root.hasPlayer && lines.length === 3 && lines[1] !== ""
        Layout.fillWidth: true
        Layout.topMargin: 14
        spacing: 2

        HText { Layout.fillWidth: true; text: lyrics.lines[0] ?? ""; ink: "c1"; elide: Text.ElideRight }
        HText { Layout.fillWidth: true; text: "» " + (lyrics.lines[1] ?? ""); ink: "c4"; elide: Text.ElideRight }
        HText { Layout.fillWidth: true; text: lyrics.lines[2] ?? ""; ink: "c1"; elide: Text.ElideRight }
    }
    RowLayout {
        visible: root.hasPlayer && Cider.active && Cider.track !== null
        Layout.topMargin: 6
        spacing: 8
        InkButton {
            text: Cider.inLibrary ? "♥ in library" : "♥ add to library"
            ink: Cider.inLibrary ? "c1" : "c3"
            enabled: !Cider.inLibrary
            onClicked: Cider.addToLibrary()
        }
        HText { text: "via cider"; role: "caption"; ink: "c1" }
    }

    // ---- players ----
    HRule {
        visible: root.hasPlayer
        Layout.fillWidth: true
        Layout.topMargin: 14
    }
    Flow {
        visible: root.hasPlayer
        Layout.fillWidth: true
        spacing: 8

        HLabel {
            width: implicitWidth + 8
            height: Math.round(HTokens.fontLabel * 1.6) + 2 * HTokens.borderHairline
            text: "players"
        }
        Repeater {
            model: root.players
            Chip {
                required property var modelData
                text: Priority.shortName(modelData)
                interactive: true
                on: modelData === root.player
                onClicked: root.playerPicked(modelData)
            }
        }
    }
    HText {
        visible: root.hasPlayer
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: "Order: spotify and cider first, whichever played last; a browser or mpv only when neither is open."
        role: "caption"
        ink: "c1"
        wrapMode: Text.WordWrap
    }
}
