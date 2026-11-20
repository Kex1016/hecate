pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H
import "AppleLogic.js" as Logic

// claude: queue. "play something for focus" becomes a list of tracks from
// the user's own library that they look over (and can trim) before anything
// plays. Claude only picks from the titles it was sent; a track it invents
// is dropped. `play in cider` and `save as playlist` are the only presses
// that touch the player.
//
//   AppleQueueCard { claude: claudeRunner; cider: Cider; request: "play something for focus" }
//
// `cider` is the Cider service: `library` (list of { id, title, artist,
// album, seconds }, filled by `loadLibrary()`) and `playQueue(tracks)`.
// `savePlaylist(name, tracks)` is optional: without it (Cider's RPC has no
// playlist call) the card has no "save as playlist" press.
H.HWindow {
    id: root

    property var claude: null
    property var cider: null
    property string request: ""
    // pending · thinking · proposed · played · saved · error
    property string phase: "pending"
    property var queue: null
    property string message: ""
    property bool expanded: false

    readonly property var libCtx: Logic.libraryContext(root.cider ? root.cider.library : [])
    readonly property int libTotal: root.cider && root.cider.library ? root.cider.library.length : 0
    readonly property var tracks: root.queue ? root.queue.tracks : []
    readonly property int shownRows: root.expanded ? root.tracks.length : Math.min(2, root.tracks.length)
    readonly property bool canSave: root.cider !== null && typeof root.cider.savePlaylist === "function"

    signal dismissed()

    label: "claude: queue"
    variant: phase === "error" ? "love" : phase === "proposed" ? "gold" : phase === "played" || phase === "saved" ? "foam" : "pine"
    implicitWidth: 480
    focus: true
    hints: phase === "pending" ? [{ key: "enter", text: "ask" }, { key: "esc", text: "close" }]
        : phase === "proposed" ? [{ key: "enter", text: "play" }, { key: "esc", text: "no" }]
        : [{ key: "esc", text: "close" }]
    note: phase === "pending" ? "nothing sent yet" : "sent: your words, " + Math.min(root.libTotal, root.libCtx.length) + " titles"

    function reset(text) {
        root.request = text ?? "";
        root.queue = null;
        root.message = "";
        root.expanded = false;
        root.phase = "pending";
        // the titles Claude may choose from; nothing is sent until the press
        if (root.cider && typeof root.cider.loadLibrary === "function")
            root.cider.loadLibrary();
        root.forceActiveFocus();
    }

    function ask() {
        if ((root.phase !== "pending" && root.phase !== "error") || root.request.trim() === "" || !root.claude)
            return;
        if (root.libCtx.length === 0) {
            root.message = root.cider && root.cider.libraryLoading ? "still reading your library from cider"
                : root.cider && root.cider.open === false ? "cider is not running" : "cider has no library to pick from yet";
            root.phase = "error";
            return;
        }
        root.message = "";
        root.phase = "thinking";
        root.claude.run(Logic.queuePrompt(root.request, root.libCtx));
    }

    function remove(index) {
        if (!root.queue || index < 0 || index >= root.queue.tracks.length)
            return;
        const tracks = root.queue.tracks.filter((t, i) => i !== index);
        let seconds = 0;
        for (const t of tracks)
            seconds += t.seconds || 0;
        root.queue = Object.assign({}, root.queue, { tracks, seconds });
        if (tracks.length === 0)
            root.no();
    }

    function play() {
        if (root.phase !== "proposed" || !root.queue)
            return;
        if (!root.cider || typeof root.cider.playQueue !== "function") {
            root.message = "cider is not available";
            root.phase = "error";
            return;
        }
        root.cider.playQueue(root.queue.tracks);
        root.phase = "played";
    }

    function save() {
        if (root.phase !== "proposed" || !root.queue)
            return;
        if (!root.cider || typeof root.cider.savePlaylist !== "function") {
            root.message = "cider cannot save playlists";
            root.phase = "error";
            return;
        }
        root.cider.savePlaylist(root.queue.name, root.queue.tracks);
        root.phase = "saved";
    }

    function no() {
        root.queue = null;
        root.phase = "pending";
        root.dismissed();
    }

    Connections {
        target: root.claude
        enabled: root.phase === "thinking"
        function onFinished(ok, text, error) {
            if (!ok) {
                root.message = error;
                root.phase = "error";
                return;
            }
            const r = Logic.normalizeQueue(Logic.extractJson(text), root.libCtx);
            if (!r.ok) {
                root.message = r.error;
                root.phase = "error";
                return;
            }
            root.queue = r.queue;
            root.expanded = false;
            root.phase = "proposed";
        }
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (root.phase === "proposed") root.play();
            else root.ask();
            event.accepted = true;
        } else if (event.key === Qt.Key_Escape) {
            if (root.phase === "proposed") root.no();
            else root.dismissed();
            event.accepted = true;
        }
    }

    H.HText {
        Layout.fillWidth: true
        text: "<font color=\"" + H.HTokens.gold + "\">?</font> " + root.request.replace(/&/g, "&amp;").replace(/</g, "&lt;")
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
    }

    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 6
        visible: root.phase === "pending" || root.phase === "thinking"
        H.HLabel { text: "sending"; height: 22; verticalAlignment: Text.AlignVCenter }
        H.Chip { text: "your words"; on: true }
        H.Chip { text: Logic.queueSummary(root.libCtx, root.libTotal).replace("sending ", ""); on: true }
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 6
        visible: root.phase === "thinking"
        text: "asking claude…"
        ink: "muted"
    }

    // the queue, to look over
    ColumnLayout {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 0
        visible: root.queue !== null && (root.phase === "proposed" || root.phase === "played" || root.phase === "saved")

        Repeater {
            model: root.tracks.slice(0, root.shownRows)
            H.ListRow {
                id: row
                required property var modelData
                required property int index
                Layout.fillWidth: true
                text: Logic.trackLine(modelData)
                ink: "text"
                selected: false
                H.InkButton {
                    visible: root.phase === "proposed"
                    text: "x"
                    flat: true
                    ink: "c1"
                    onClicked: root.remove(row.index)
                }
            }
        }

        H.ListRow {
            Layout.fillWidth: true
            visible: root.tracks.length > 2
            text: root.expanded ? "show fewer" : "+ " + (root.tracks.length - 2) + " more, " + Logic.duration(root.queue ? root.queue.seconds : 0)
            ink: "muted"
            onClicked: root.expanded = !root.expanded
        }
        H.HText {
            Layout.fillWidth: true
            Layout.topMargin: 4
            visible: root.queue !== null && root.queue.dropped > 0
            text: root.queue ? root.queue.dropped + " suggested " + (root.queue.dropped === 1 ? "track was" : "tracks were") + " not in your library and left out" : ""
            role: "caption"
            ink: "inkMuted"
        }
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.phase === "played" || root.phase === "saved"
        text: root.phase === "played" ? "playing in cider" : "saved as " + (root.queue ? root.queue.name : "")
        ink: "foam"
    }

    H.HText {
        Layout.fillWidth: true
        Layout.topMargin: 4
        visible: root.message !== ""
        text: root.message
        ink: "love"
        wrapMode: Text.WordWrap
    }

    RowLayout {
        Layout.topMargin: 8
        spacing: 8
        visible: root.phase === "pending" || root.phase === "error" || root.phase === "proposed"
        H.InkButton {
            visible: root.phase === "pending" || root.phase === "error"
            text: root.phase === "error" ? "try again" : "ask claude"
            ink: "c3"
            enabled: root.request.trim() !== "" && root.claude !== null
            onClicked: root.ask()
        }
        H.InkButton { visible: root.phase === "proposed"; text: "play in cider"; ink: "c3"; onClicked: root.play() }
        H.InkButton { visible: root.phase === "proposed" && root.canSave; text: "save as playlist"; ink: "c1"; onClicked: root.save() }
        H.InkButton { text: root.phase === "proposed" ? "no" : "close"; ink: "c1"; onClicked: root.phase === "proposed" ? root.no() : root.dismissed() }
    }
}
