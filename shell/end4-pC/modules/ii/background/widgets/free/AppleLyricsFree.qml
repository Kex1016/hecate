pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.services
import qs.services.apple
import qs.modules.common.widgets.haiiro as H

// cider: lyrics, free · the line being sung is the biggest thing on it, with the rose » and
// the words already sung in text, the rest subtle; the lines around it sit above and below.
// Lines cut from one to the next. Between lines a long gap is "instrumental": three squares
// and the time until the voice is back. Plain text is a ruled note that scrolls with the
// song. With `big` only the current line shows, in Jacquard. A song without lyrics shows one
// line and the widget hides itself until the next song. From Cider (services/apple/Cider.qml).
H.FreeWidget {
    id: root

    // only the current line, in Jacquard at 72px
    property bool big: false
    // a gap between two lines this long (seconds) is an instrumental
    property real gapAt: 6
    // the ruled note shows this many lines
    property int noteLines: 4

    readonly property var player: MprisController.activePlayer
    readonly property string playerName: player ? String(player.identity).toLowerCase() : "media"
    readonly property bool problem: !Cider.open || Cider.status === "off" || Cider.status === "token"
    readonly property bool synced: !problem && Cider.active && Cider.lyricsStatus === "ok" && Cider.hasLines
    readonly property bool plain: !problem && Cider.active && Cider.lyricsStatus === "unsynced" && Cider.hasLines
    readonly property bool loading: !problem && Cider.active && (Cider.lyricsStatus === "loading" || Cider.lyricsStatus === "")
    // a song with nothing to show
    readonly property bool none: !root.synced && !root.plain && !root.loading
    property real now: 0
    property int step: 0
    property bool gone: false

    // position and gap, refreshed while a line can change
    readonly property int idx: Cider.lineIndex
    readonly property var line: root.synced && root.idx >= 0 ? Cider.lines[root.idx] : null
    readonly property var upcoming: root.synced ? Cider.lines[root.idx + 1] ?? null : null
    readonly property real lineEnd: root.line ? (isNaN(root.line.end) ? (root.upcoming?.time ?? Infinity) : root.line.end) : 0
    // instrumental: past the end of this line (or before the first) and the next is a while off
    readonly property bool instrumental: {
        if (!root.synced) return false;
        const t = root.now;
        if (root.idx < 0) return Cider.lines.length > 0 && Cider.lines[0].time > root.gapAt;
        if (!root.line || !root.upcoming || isNaN(root.line.end)) return false;
        return t > root.line.end && root.upcoming.time - root.line.end > root.gapAt;
    }
    readonly property real untilVoice: root.synced ? (root.idx < 0 ? (Cider.lines[0]?.time ?? 0) : (root.upcoming?.time ?? 0)) - root.now : 0
    // share of the current line already sung, 0-1
    readonly property real sung: {
        if (!root.line) return 0;
        const span = root.lineEnd - root.line.time;
        return span > 0 && isFinite(span) ? Math.max(0, Math.min(1, (root.now - root.line.time) / span)) : 0;
    }

    label: `${root.playerName}: lyrics` + (root.big ? " --big" : "")
    mark: "♪"
    visible: !root.gone
    implicitWidth: root.big ? 480 : 560

    function clock(seconds) {
        const s = Math.max(0, Math.floor(seconds || 0));
        return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
    }
    // the first `sung` share of the words, then the rest
    function split(text) {
        const words = text.split(" ");
        const n = Math.round(root.sung * words.length);
        return [words.slice(0, n).join(" "), words.slice(n).join(" ")];
    }
    function esc(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
    }
    function lyric(text) {
        const parts = root.split(text);
        return `${root.esc(parts[0])}${parts[0] && parts[1] ? " " : ""}<font color="${H.HTokens.subtle}">${root.esc(parts[1])}</font>`;
    }

    // holds the service's faster polling and the line timer while this is up
    property bool held: false
    function hold() {
        if (root.live && !root.held) { Cider.consumers++; root.held = true; }
        else if (!root.live && root.held) { Cider.consumers--; root.held = false; }
    }
    onLiveChanged: hold()
    Component.onCompleted: hold()
    Component.onDestruction: if (root.held) Cider.consumers--

    Timer {
        running: root.live && (root.synced || root.plain)
        interval: 100
        repeat: true
        triggeredOnStart: true
        onTriggered: root.now = Cider.currentPosition()
    }
    Timer {
        running: root.live && root.instrumental
        interval: 1000
        repeat: true
        onTriggered: root.step = (root.step + 1) % 3
    }
    // a song without lyrics: one line, then the widget hides until the next song
    Timer {
        running: root.live && root.none && !root.problem
        interval: 8000
        onTriggered: root.gone = true
    }
    Connections {
        target: Cider
        function onTrackIdChanged() { root.gone = false; }
    }
    onProblemChanged: root.gone = false

    // ── the song ──────────────────────────────────────────────────────────────────

    H.FreeText {
        visible: (root.synced || root.plain) && !root.big && !!Cider.track
        role: "code"
        ink: "subtle"
        text: `${Cider.track?.artist ?? ""} · ${Cider.track?.name ?? ""}`
    }

    // ── synced ────────────────────────────────────────────────────────────────────

    H.FreeText {
        Layout.fillWidth: true
        Layout.topMargin: 8
        visible: root.synced && !root.big && !root.instrumental
        role: "code"
        elide: Text.ElideRight
        text: root.idx > 0 ? Cider.lineAt(root.idx - 1) : ""
    }
    // the line being sung
    RowLayout {
        Layout.fillWidth: true
        visible: root.synced && !root.big && !root.instrumental
        spacing: 12
        H.FreeText { role: "nav"; ink: "rose"; text: "»"; visible: root.line !== null }
        H.FreeText {
            Layout.fillWidth: true
            role: "nav"
            elide: Text.ElideRight
            textFormat: Text.StyledText
            text: root.line ? root.lyric(root.line.text) : ""
        }
    }
    H.FreeText {
        Layout.fillWidth: true
        visible: root.synced && !root.big && !root.instrumental
        role: "code"
        elide: Text.ElideRight
        text: Cider.lineAt(Math.max(0, root.idx) + 1)
    }
    H.FreeText {
        Layout.fillWidth: true
        visible: root.synced && !root.big && !root.instrumental
        role: "code"
        ink: "muted"
        elide: Text.ElideRight
        text: Cider.lineAt(Math.max(0, root.idx) + 2)
    }

    // the line, huge
    H.FreeText {
        Layout.fillWidth: true
        visible: root.synced && root.big
        role: "display"
        size: 72
        wrapMode: Text.Wrap
        textFormat: Text.StyledText
        text: root.line ? root.lyric(root.line.text) : ""
    }

    // instrumental: the line before, three squares, the time left and what comes next
    H.FreeText {
        Layout.fillWidth: true
        Layout.topMargin: 8
        visible: root.instrumental && !root.big
        role: "code"
        elide: Text.ElideRight
        text: root.line?.text ?? ""
    }
    H.PixelCells {
        visible: root.instrumental
        Layout.topMargin: 6
        count: 3
        lit: root.step
        ink: "rose"
        cell: 10
        gap: 6
    }
    H.FreeText {
        visible: root.instrumental
        role: "caption"
        ink: "subtle"
        text: `instrumental · ${root.clock(root.untilVoice)}`
    }
    RowLayout {
        visible: root.instrumental && !root.big
        spacing: 8
        H.FreeText { role: "code"; ink: "subtle"; text: "next:" }
        H.FreeText {
            Layout.fillWidth: true
            role: "code"
            elide: Text.ElideRight
            text: root.idx < 0 ? Cider.lineAt(0) : (root.upcoming?.text ?? "")
        }
    }

    // ── not synced ────────────────────────────────────────────────────────────────

    H.RuledNote {
        Layout.fillWidth: true
        Layout.topMargin: 8
        visible: root.plain
        role: "code"
        minLines: root.noteLines
        // it scrolls with the song: the window of lines the position has reached
        readonly property int total: Cider.lines.length
        readonly property real share: Cider.track && Cider.track.durationMs > 0 ? Math.min(1, root.now * 1000 / Cider.track.durationMs) : 0
        readonly property int first: Math.floor(share * Math.max(0, total - root.noteLines + 1))
        text: Cider.lines.slice(first, first + root.noteLines).map(l => l.text).join("\n")
    }

    // ── progress, heart and what this is ──────────────────────────────────────────

    H.PlayLine {
        Layout.fillWidth: true
        Layout.topMargin: 6
        visible: root.synced && !root.big && !root.instrumental
        progress: Cider.track && Cider.track.durationMs > 0 ? Math.min(1, root.now * 1000 / Cider.track.durationMs) : 0
    }
    RowLayout {
        visible: (root.synced || root.plain) && !root.big && !root.instrumental && Cider.reachable && Cider.track !== null
        spacing: 8
        H.InkButton {
            visible: root.synced
            ink: Cider.inLibrary ? "c1" : "c3"
            role: "code"
            text: Cider.inLibrary ? "♥ in library" : "♥ add to library"
            enabled: !Cider.inLibrary
            onClicked: Cider.addToLibrary()
        }
        H.FreeText {
            role: "caption"
            ink: "subtle"
            text: root.synced ? `synced · via cider · ${root.clock(root.now)} / ${root.clock((Cider.track?.durationMs ?? 0) / 1000)}`
                : "not synced · scrolls with the song"
        }
    }

    // ── nothing to show ───────────────────────────────────────────────────────────

    H.FreeText {
        Layout.fillWidth: true
        visible: root.problem || root.loading || root.none
        role: "code"
        ink: "muted"
        wrapMode: Text.Wrap
        text: !Cider.open ? "open cider for lyrics"
            : Cider.status === "off" ? "turn on cider's rpc server: settings › connectivity"
            : Cider.status === "token" ? "cider wants a token: apple.cider.token, or the keyring"
            : !Cider.active ? "no lyrics here · they come from cider"
            : root.loading ? "…"
            : "no lyrics for this one"
    }
    H.FreeText {
        visible: root.none && !root.problem
        role: "caption"
        ink: "subtle"
        text: "the widget hides itself after a song without them"
    }
}
