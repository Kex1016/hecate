pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.services

/**
 * Cider (cider.sh), the Apple Music client, through its local RPC server
 * (http://localhost:10767, Settings → Connectivity → WebSockets/RPC in Cider).
 *
 * MPRIS already gives the shell the track and the transport; this adds what only Cider knows:
 * whether the track is in your library (and adding it), synced lyrics from Apple Music, and
 * the queue (reading it, playing next or later, removing, playing a list of songs, which is
 * what "play in cider" on a Claude queue does).
 *
 * Lyrics: Cider's own lyrics endpoint does nothing, so they come from the Apple Music API
 * through Cider's `amapi/run-v3` passthrough (`/v1/catalog/<storefront>/songs/<id>/lyrics`,
 * TTML), signed with Cider's own Apple Music session. Lines carry their start times.
 *
 * The token: Cider asks external apps for one unless "require token" is off. It is
 * `apple.cider.token`, else the keyring item `application hecate service cider`
 * (`secret-tool store --label "Cider" application hecate service cider`).
 *
 * Nothing is asked while no Cider MPRIS player exists.
 */
Singleton {
    id: root

    readonly property string base: Config.options?.apple?.cider?.url || "http://localhost:10767"
    readonly property string configuredToken: Config.options?.apple?.cider?.token ?? ""
    property string keyringToken: ""
    readonly property string token: root.configuredToken || root.keyringToken

    // The Cider MPRIS player, if one is open.
    readonly property var player: MprisController.players.find(p => root.isCiderPlayer(p)) ?? null
    readonly property bool open: root.player !== null
    // Cider is the player the shell shows.
    readonly property bool active: root.open && MprisController.activePlayer === root.player
    // The RPC server answered the last request.
    property bool reachable: false
    // "ok" | "token" (Cider wants a token) | "off" (no RPC server: turn it on in Cider) | ""
    property string status: ""

    // Panels showing lyrics or the queue hold this up: they are polled faster then.
    property int consumers: 0

    // ── now playing ───────────────────────────────────────────────────────────────

    property var track: null
    // { id, kind, name, artist, album, durationMs, inLibrary, inFavorites, hasLyrics, synced }
    readonly property string trackId: root.track?.id ?? ""
    readonly property bool inLibrary: root.track?.inLibrary ?? false
    property real position: 0         // seconds, at positionAt
    property double positionAt: 0
    property bool playing: false

    function isCiderPlayer(p) {
        if (!p) return false;
        const id = `${p.identity ?? ""} ${p.desktopEntry ?? ""} ${p.dbusName ?? ""}`.toLowerCase();
        return id.includes("cider");
    }

    function currentPosition() {
        return root.playing ? root.position + (Date.now() - root.positionAt) / 1000 : root.position;
    }

    // ── requests ──────────────────────────────────────────────────────────────────

    function request(method, path, body, done) {
        const xhr = new XMLHttpRequest();
        xhr.open(method, root.base + path);
        xhr.timeout = 3000;
        if (root.token) xhr.setRequestHeader("apptoken", root.token);
        if (body !== undefined && body !== null) xhr.setRequestHeader("Content-Type", "application/json");
        xhr.onreadystatechange = () => {
            if (xhr.readyState !== XMLHttpRequest.DONE) return;
            if (xhr.status === 0) {
                root.reachable = false;
                root.status = "off";
                done?.(null, 0);
                return;
            }
            root.reachable = true;
            if (xhr.status === 401 || xhr.status === 403) {
                root.status = "token";
                done?.(null, xhr.status);
                return;
            }
            root.status = "ok";
            let json = null;
            try {
                json = xhr.responseText ? JSON.parse(xhr.responseText) : null;
            } catch (e) {
                json = null;
            }
            done?.(json, xhr.status);
        };
        xhr.send(body !== undefined && body !== null ? JSON.stringify(body) : null);
    }

    function post(path, body, done) {
        root.request("POST", path, body ?? null, done);
    }

    // Apple Music API through Cider's session. `done(json)` gets the API's own answer.
    function amapi(path, done) {
        root.post("/api/v1/amapi/run-v3", { path: path }, (json, code) => done(json?.data ?? json, code));
    }

    function pollNowPlaying() {
        if (!root.open) return;
        root.request("GET", "/api/v1/playback/now-playing", null, (json, code) => {
            const info = json?.info;
            if (!info || code === 204 || code === 404) {
                if (code !== 0) root.track = null;
                return;
            }
            const id = String(info.playParams?.id ?? info.playParams?.catalogId ?? "");
            const next = {
                id: id,
                kind: info.playParams?.kind ?? "song",
                name: info.name ?? "",
                artist: info.artistName ?? "",
                album: info.albumName ?? "",
                durationMs: info.durationInMillis ?? 0,
                inLibrary: !!info.inLibrary,
                inFavorites: !!info.inFavorites,
                hasLyrics: info.hasLyrics !== false,
                synced: !!info.hasTimeSyncedLyrics,
            };
            // A just-added track keeps its heart until Cider agrees.
            if (root.track && root.track.id === id && root.addedId === id) next.inLibrary = true;
            if (!root.track || root.track.id !== id) root.addedId = "";
            const changed = !root.track || root.track.id !== id;
            root.track = next;
            root.position = info.currentPlaybackTime ?? 0;
            root.positionAt = Date.now();
            root.playing = root.player?.isPlaying ?? false;
            if (changed) root.loadLyrics();
            root.updateLine();
        });
    }

    // ── library ───────────────────────────────────────────────────────────────────

    property string addedId: ""

    function addToLibrary() {
        if (!root.track) return;
        const id = root.track.id;
        root.addedId = id;
        root.track = Object.assign({}, root.track, { inLibrary: true });
        root.post("/api/v1/playback/add-to-library", null, (json, code) => {
            if (code < 200 || code >= 300) {
                root.addedId = "";
                if (root.track?.id === id) root.track = Object.assign({}, root.track, { inLibrary: false });
            }
        });
    }

    // Your Apple Music library's songs, for the Claude queue card, which may only choose from
    // them: [{ id, type, title, artist, album, seconds }]. Fetched on loadLibrary(), newest
    // additions first, at most `libraryMax`. Songs that are also in the catalogue carry the
    // catalogue id, which Cider plays like any other; the rest play by their library id.
    property var library: []
    property bool libraryLoading: false
    property double libraryAt: 0
    readonly property int libraryMax: 300

    function loadLibrary(force) {
        if (!root.open || root.libraryLoading) return;
        // once every ten minutes is plenty for a list the queue card reads
        if (!force && root.library.length > 0 && Date.now() - root.libraryAt < 10 * 60 * 1000) return;
        root.libraryLoading = true;
        const got = [];
        const page = offset => root.amapi(`/v1/me/library/songs?limit=100&offset=${offset}`, json => {
            const data = json?.data ?? [];
            for (const s of data) {
                const a = s.attributes ?? {};
                const catalogId = a.playParams?.catalogId ?? "";
                got.push({
                    id: String(catalogId || s.id),
                    type: catalogId ? "songs" : (s.type ?? "library-songs"),
                    title: a.name ?? "",
                    artist: a.artistName ?? "",
                    album: a.albumName ?? "",
                    seconds: Math.round((a.durationInMillis ?? 0) / 1000),
                });
            }
            if (data.length === 100 && got.length < root.libraryMax && json?.next) {
                page(offset + 100);
                return;
            }
            root.library = got.slice(0, root.libraryMax);
            root.libraryAt = Date.now();
            root.libraryLoading = false;
        });
        page(0);
    }

    // The queue card's "play in cider": the tracks it chose, in order, replacing what plays.
    function playQueue(tracks) {
        root.playItems((tracks ?? []).map(t => ({ id: t.id, type: t.type ?? "songs" })));
    }
    // There is no savePlaylist(): Cider's RPC has no playlist call and run-v3 only passes GETs,
    // so the card hides its "save as playlist" press.

    // 1 love, -1 dislike, 0 neither
    function rate(rating) {
        root.post("/api/v1/playback/set-rating", { rating: rating });
    }

    // ── lyrics ────────────────────────────────────────────────────────────────────

    // [{ time, end (seconds), text }], in order; empty while there are none.
    property var lines: []
    property int lineIndex: -1
    // "" | "loading" | "ok" | "none" (Apple Music has none) | "unsynced" (text without times)
    property string lyricsStatus: ""
    readonly property bool hasLines: root.lines.length > 0
    readonly property string previousLine: root.lineAt(root.lineIndex - 1)
    readonly property string currentLine: root.lineAt(root.lineIndex)
    readonly property string nextLine: root.lineAt(root.lineIndex + 1)
    property string storefront: ""

    function lineAt(i) {
        return i >= 0 && i < root.lines.length ? root.lines[i].text : "";
    }

    function loadLyrics() {
        root.lines = [];
        root.lineIndex = -1;
        const track = root.track;
        if (!track || !track.id || !track.hasLyrics) {
            root.lyricsStatus = track ? "none" : "";
            return;
        }
        root.lyricsStatus = "loading";
        const withStorefront = sf => {
            const id = track.id;
            root.amapi(`/v1/catalog/${sf}/songs/${id}/lyrics`, json => {
                if (root.track?.id !== id) return;
                const ttml = json?.data?.[0]?.attributes?.ttml ?? "";
                const parsed = root.parseTtml(ttml);
                root.lines = parsed.lines;
                root.lyricsStatus = parsed.lines.length === 0 ? "none" : (parsed.timed ? "ok" : "unsynced");
                root.updateLine();
            });
        };
        if (root.storefront) {
            withStorefront(root.storefront);
            return;
        }
        root.amapi("/v1/me/storefront", json => {
            root.storefront = json?.data?.[0]?.id ?? "us";
            withStorefront(root.storefront);
        });
    }

    // "1:02.345", "00:01:02.345", "62.345", "62.345s" -> seconds
    function ttmlTime(value) {
        if (!value) return NaN;
        const v = String(value).trim().replace(/s$/, "");
        const parts = v.split(":").map(Number);
        if (parts.some(isNaN)) return NaN;
        return parts.reduce((total, part) => total * 60 + part, 0);
    }

    function decodeEntities(s) {
        return s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"").replace(/&apos;/g, "'")
            .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(parseInt(n))).replace(/&amp;/g, "&");
    }

    // The <p> lines of Apple Music's TTML, with their begin and end times. Syllable-timed
    // lyrics put their words in <span>s inside each <p>; only the line times are used.
    function parseTtml(ttml) {
        const out = [];
        let timed = true;
        const re = /<p\b([^>]*)>([\s\S]*?)<\/p>/g;
        let m;
        while ((m = re.exec(String(ttml))) !== null) {
            const attrs = m[1];
            const begin = root.ttmlTime((attrs.match(/\bbegin="([^"]*)"/) ?? [])[1]);
            const end = root.ttmlTime((attrs.match(/\bend="([^"]*)"/) ?? [])[1]);
            // Background vocals (ttm:role="x-bg") stay out of the line.
            const body = m[2].replace(/<span[^>]*ttm:role="x-bg"[^>]*>[\s\S]*?<\/span>/g, "");
            const text = root.decodeEntities(body.replace(/<br\s*\/?>/g, " ").replace(/<[^>]+>/g, "")).replace(/\s+/g, " ").trim();
            if (text.length === 0) continue;
            if (isNaN(begin)) timed = false;
            out.push({ time: isNaN(begin) ? 0 : begin, end: isNaN(end) ? NaN : end, text: text });
        }
        return { lines: out, timed: timed && out.length > 0 };
    }

    // The line being sung: the last that started (a little early, so it lands on the beat).
    function indexAt(pos) {
        let low = 0, high = root.lines.length - 1, result = -1;
        while (low <= high) {
            const mid = (low + high) >> 1;
            if (root.lines[mid].time <= pos) {
                result = mid;
                low = mid + 1;
            } else {
                high = mid - 1;
            }
        }
        return result;
    }

    function updateLine() {
        if (root.lyricsStatus !== "ok") {
            root.lineIndex = root.lyricsStatus === "unsynced" ? 0 : -1;
            return;
        }
        const idx = root.indexAt(root.currentPosition() + 0.15);
        if (idx !== root.lineIndex) root.lineIndex = idx;
    }

    Timer {
        running: root.lyricsStatus === "ok" && root.playing && root.consumers > 0
        interval: 100
        repeat: true
        onTriggered: root.updateLine()
    }

    // ── queue ─────────────────────────────────────────────────────────────────────

    // What plays after the current track: [{ index (Cider's, 1-based, history included),
    // id, type, name, artist, durationMs }]
    property var upNext: []

    function refreshQueue() {
        if (!root.open) return;
        root.request("GET", "/api/v1/playback/queue", null, (json, code) => {
            const items = Array.isArray(json) ? json : (json?.queue ?? []);
            const current = items.findIndex(i => i?._state?.current === 2);
            const after = items.map((item, i) => ({ item: item, index: i + 1 })).slice(current + 1);
            root.upNext = after.map(e => ({
                index: e.index,
                id: String(e.item.attributes?.playParams?.id ?? e.item.id ?? ""),
                type: e.item.type ?? "songs",
                name: e.item.attributes?.name ?? "",
                artist: e.item.attributes?.artistName ?? "",
                durationMs: e.item.attributes?.durationInMillis ?? 0,
            }));
        });
    }

    function removeFromQueue(index) {
        root.post("/api/v1/playback/queue/remove-by-index", { index: index }, () => root.refreshQueue());
    }
    function moveInQueue(from, to) {
        root.post("/api/v1/playback/queue/move-to-position", { startIndex: from, destinationIndex: to }, () => root.refreshQueue());
    }
    function clearQueue() {
        root.post("/api/v1/playback/queue/clear-queue", null, () => root.refreshQueue());
    }
    // item: { type: "songs" | "albums" | "playlists" | ..., id }
    function playNext(item) {
        root.post("/api/v1/playback/play-next", { type: item.type ?? "songs", id: String(item.id) }, () => root.refreshQueue());
    }
    function playLater(item) {
        root.post("/api/v1/playback/play-later", { type: item.type ?? "songs", id: String(item.id) }, () => root.refreshQueue());
    }
    // Play a list now: the first item replaces what is playing, the rest follow in order.
    function playItems(items) {
        const list = (items ?? []).filter(i => i && i.id);
        if (list.length === 0) return;
        root.post("/api/v1/playback/play-item", { type: list[0].type ?? "songs", id: String(list[0].id) }, () => {
            const rest = list.slice(1);
            const step = () => {
                const item = rest.shift();
                if (!item) {
                    root.refreshQueue();
                    return;
                }
                root.post("/api/v1/playback/play-later", { type: item.type ?? "songs", id: String(item.id) }, step);
            };
            step();
        });
    }
    function playUrl(url) {
        root.post("/api/v1/playback/play-url", { url: url });
    }

    // Songs in the Apple Music catalogue, for turning "artist · title" into something to
    // play: done([{ id, type: "songs", name, artist, durationMs }])
    function searchSongs(term, done, limit) {
        const go = sf => root.amapi(`/v1/catalog/${sf}/search?types=songs&limit=${limit ?? 5}&term=${encodeURIComponent(term)}`, json => {
            const songs = json?.results?.songs?.data ?? [];
            done(songs.map(s => ({ id: String(s.id), type: "songs", name: s.attributes?.name ?? "",
                artist: s.attributes?.artistName ?? "", durationMs: s.attributes?.durationInMillis ?? 0 })));
        });
        if (root.storefront) go(root.storefront);
        else root.amapi("/v1/me/storefront", json => {
            root.storefront = json?.data?.[0]?.id ?? "us";
            go(root.storefront);
        });
    }

    // ── polling ───────────────────────────────────────────────────────────────────

    Timer {
        running: root.open
        interval: root.consumers > 0 ? 2000 : 10000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.pollNowPlaying()
    }
    Timer {
        running: root.open && root.consumers > 0
        interval: 15000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refreshQueue()
    }

    Connections {
        target: root.player
        function onPostTrackChanged() {
            root.pollNowPlaying();
            if (root.consumers > 0) root.refreshQueue();
        }
        function onIsPlayingChanged() {
            root.position = root.currentPosition();
            root.positionAt = Date.now();
            root.playing = root.player?.isPlaying ?? false;
        }
    }
    onOpenChanged: {
        if (!root.open) {
            root.track = null;
            root.lines = [];
            root.lyricsStatus = "";
            root.upNext = [];
            root.library = [];
            root.libraryAt = 0;
            root.libraryLoading = false;
            root.reachable = false;
            root.status = "";
        }
    }

    Process {
        running: true
        command: ["sh", "-c", "command -v secret-tool >/dev/null && secret-tool lookup application hecate service cider 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: root.keyringToken = text.trim()
        }
    }

    function load() {} // for forcing initialisation

    IpcHandler {
        target: "cider"

        function addToLibrary(): void {
            root.addToLibrary();
        }
        function status(): string {
            return JSON.stringify({ open: root.open, status: root.status, track: root.track, lyrics: root.lyricsStatus,
                line: root.currentLine, upNext: root.upNext.length });
        }
    }
}
