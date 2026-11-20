pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.services

/**
 * An iCloud Photos album as a wallpaper source (`background.source = "icloudAlbum"`,
 * `background.album`). Offered only in wallpaper mode: with the blobs there is no picture.
 *
 * icloudpd fetches the album into ~/Pictures/iCloud/<album> (originals, no videos, no live
 * photo movies, one flat folder). HEIC photos, which Qt can't read, get a JPEG copy in
 * `.converted/` beside them. Then a picture from the folder is the wallpaper, a new one every
 * `rotate` minutes; Wallpapers dithers it like any other.
 *
 * Signing in is icloudpd's: the first time (and when Apple's session runs out, about every
 * two months) `signIn()` opens a terminal where icloudpd asks for the password, keeps it in
 * the keyring, and asks for the two-factor code. Every sync after that is unattended
 * (`--password-provider keyring`); one that would need the code stops and says so (`status`
 * "signIn").
 *
 * The Apple ID is `apple.appleId`, else the user of the first iCloud calendar account
 * (~/.config/hecate/calendars.toml), the same account as the calendar.
 *
 * icloudpd reads your own albums and smart albums. Shared albums (iCloud Shared Streams) are
 * not something it can fetch.
 */
Singleton {
    id: root

    // ── settings ──────────────────────────────────────────────────────────────────
    // Config.options.apple.* once Config declares it; these are the defaults.
    readonly property string configuredAppleId: Config.options?.apple?.appleId ?? ""
    readonly property string appleId: root.configuredAppleId || root.calendarAppleId
    // Minutes between wallpapers, and hours between fetches.
    readonly property int rotateMinutes: Config.options?.apple?.photos?.rotate ?? 60
    readonly property int syncHours: Config.options?.apple?.photos?.syncHours ?? 6

    readonly property bool installed: root.tools.icloudpd === true
    readonly property string album: Config.options.background.album ?? ""
    readonly property bool selected: Config.options.background.source === "icloudAlbum"
    // The source is offered only in wallpaper mode, and works once it names an album.
    readonly property bool offered: Wallpapers.enabled
    readonly property bool active: root.offered && root.selected && root.album.length > 0 && root.appleId.length > 0

    readonly property string cookieDir: FileUtils.trimFileProtocol(`${Directories.state}/user/icloudpd`)
    readonly property string photosRoot: FileUtils.trimFileProtocol(`${Directories.pictures}/iCloud`)
    readonly property string albumDir: `${root.photosRoot}/${root.album.replace(/[\/\0]/g, "_")}`

    // status: "idle" | "syncing" | "ok" | "signIn" (icloudpd needs the password or a code) |
    // "missing" (icloudpd is not installed) | "error"
    property string status: "idle"
    property string message: ""
    property double synced: 0
    // Every picture in the album folder that Qt can show, newest first.
    property var photos: []
    readonly property int count: root.photos.length
    property var albums: []
    readonly property bool listing: listProc.running
    readonly property bool syncing: syncProc.running

    function setSource(source) {
        Config.options.background.source = source === "icloudAlbum" ? "icloudAlbum" : "folder";
    }
    function setAlbum(name) {
        Config.options.background.album = name;
    }

    // ── signing in ────────────────────────────────────────────────────────────────

    // icloudpd in a terminal: password (saved to the keyring), then the two-factor code.
    function signIn() {
        if (!root.appleId) return;
        const inner = ["icloudpd", "--auth-only", "--password-provider", "keyring", "--password-provider", "console",
            "--mfa-provider", "console", "-u", root.appleId, "--cookie-directory", root.cookieDir]
            .map(a => root.quote(a)).join(" ");
        const marker = root.quote(`${root.cookieDir}/.signed-in`);
        const script = `mkdir -p ${root.quote(root.cookieDir)}; ${inner}; date +%s > ${marker}; echo; echo "done, you can close this window"; read -r _`;
        root.openTerminal(script);
        root.signInPending = true;
    }
    property bool signInPending: false

    function quote(s) {
        return `'${String(s).replace(/'/g, "'\\''")}'`;
    }

    // A command in the user's terminal: xdg-terminal-exec when there is one, else
    // `apps.terminal` with the command after it (kitty, foot and wezterm's start take that).
    function openTerminal(script) {
        const terminal = Config.options?.apps?.terminal || "kitty -1";
        Quickshell.execDetached(["sh", "-c",
            'if command -v xdg-terminal-exec >/dev/null; then exec xdg-terminal-exec sh -c "$1"; else exec $2 sh -c "$1"; fi',
            "terminal", script, terminal]);
    }

    // ── albums ────────────────────────────────────────────────────────────────────

    function listAlbums() {
        if (!root.installed || !root.appleId || listProc.running) return;
        listProc.command = ["sh", "-c", 'mkdir -p "$2" && exec icloudpd --no-progress-bar --log-level error --password-provider keyring --mfa-provider console -u "$1" --cookie-directory "$2" --list-albums </dev/null 2>&1',
            "icloudpd", root.appleId, root.cookieDir];
        listProc.running = true;
    }

    Process {
        id: listProc
        stdout: StdioCollector {
            onStreamFinished: {
                const lines = text.split("\n").map(l => l.trim());
                const start = lines.indexOf("Albums:");
                if (start < 0) {
                    root.noteFailure(text);
                    return;
                }
                root.albums = lines.slice(start + 1).filter(l => l.length > 0);
            }
        }
    }

    // ── syncing ───────────────────────────────────────────────────────────────────

    // $1 Apple ID, $2 cookies, $3 album, $4 folder. Fetch, convert HEIC, then list what Qt
    // can show, newest first, after a "::photos" line.
    readonly property string syncScript: [
        'set -u',
        'mkdir -p "$2" "$4" "$4/.converted" || exit 1',
        'icloudpd --no-progress-bar --log-level info --password-provider keyring --mfa-provider console \\',
        '    -u "$1" --cookie-directory "$2" -d "$4" -a "$3" --size original --skip-videos --skip-live-photos \\',
        '    --folder-structure none </dev/null 2>&1',
        'status=$?',
        'for f in "$4"/*.[hH][eE][iI][cC] "$4"/*.[hH][eE][iI][fF]; do',
        '    [ -e "$f" ] || continue',
        '    out="$4/.converted/$(basename "$f").jpg"',
        '    [ -s "$out" ] || magick "$f[0]" -auto-orient -quality 92 "$out" 2>/dev/null || rm -f "$out"',
        'done',
        'echo "::status $status"',
        'echo "::photos"',
        'ls -1t "$4"/*.[jJ][pP][gG] "$4"/*.[jJ][pP][eE][gG] "$4"/*.[pP][nN][gG] "$4"/*.[wW][eE][bB][pP] "$4"/.converted/*.jpg 2>/dev/null',
    ].join("\n")

    function sync() {
        if (!root.active || syncProc.running) return;
        if (!root.installed) {
            root.status = "missing";
            return;
        }
        root.status = "syncing";
        root.message = "";
        syncProc.command = ["sh", "-c", root.syncScript, "photos", root.appleId, root.cookieDir, root.album, root.albumDir];
        syncProc.running = true;
    }

    Process {
        id: syncProc
        stdout: StdioCollector {
            onStreamFinished: {
                const all = text.split("\n");
                const marker = all.indexOf("::photos");
                const log = marker >= 0 ? all.slice(0, marker) : all;
                const statusLine = log.find(l => l.startsWith("::status ")) ?? "::status 1";
                const ok = statusLine.trim() === "::status 0";
                root.photos = marker >= 0 ? all.slice(marker + 1).map(l => l.trim()).filter(l => l.length > 0) : root.photos;
                if (ok) {
                    root.status = "ok";
                    root.message = "";
                    root.synced = Date.now();
                } else {
                    root.noteFailure(log.join("\n"));
                }
                if (root.active && root.photos.length > 0 && !root.photos.includes(Config.options.background.wallpaperPath))
                    root.next();
            }
        }
    }

    function noteFailure(log) {
        if (/two-factor|two-step|password|authenticat|Invalid email|EOF when reading|No password/i.test(log)) {
            root.status = "signIn";
            root.message = "sign in to iCloud again";
        } else if (/not found|No album|KeyError/i.test(log) && root.album) {
            root.status = "error";
            root.message = `no album called ${root.album}`;
        } else {
            root.status = "error";
            const last = log.split("\n").map(l => l.trim()).filter(l => l.length > 0 && !l.startsWith("::")).pop() ?? "";
            root.message = last.slice(0, 160);
        }
    }

    // ── rotating ──────────────────────────────────────────────────────────────────

    // Another picture from the album, never the one showing.
    function next() {
        if (!root.active || root.photos.length === 0) return;
        const current = Config.options.background.wallpaperPath;
        const others = root.photos.filter(p => p !== current);
        const pick = (others.length > 0 ? others : root.photos)[Math.floor(Math.random() * (others.length || root.photos.length))];
        Wallpapers.apply(pick);
    }

    Timer {
        running: root.active && root.rotateMinutes > 0 && root.photos.length > 1
        interval: Math.max(1, root.rotateMinutes) * 60 * 1000
        repeat: true
        onTriggered: root.next()
    }

    Timer {
        running: root.active && root.syncHours > 0
        interval: Math.max(1, root.syncHours) * 60 * 60 * 1000
        repeat: true
        // A session that ran out waits for signIn(): every attempt would send another code.
        onTriggered: if (root.status !== "signIn") root.sync()
    }

    onActiveChanged: if (root.active) root.sync()
    onAlbumChanged: {
        root.photos = [];
        if (root.active) root.sync();
    }

    // ── tools and the Apple ID from the calendars ───────────────────────────────

    property var tools: ({})
    property string calendarAppleId: ""

    Process {
        id: toolsProc
        running: true
        command: ["sh", "-c", 'for t in icloudpd magick; do command -v "$t" >/dev/null && echo "$t"; done']
        stdout: StdioCollector {
            onStreamFinished: {
                const found = {};
                for (const t of text.split("\n").map(l => l.trim()).filter(l => l)) found[t] = true;
                root.tools = found;
                if (root.active) root.sync();
            }
        }
    }

    FileView {
        path: Directories.calendarsPath
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.calendarAppleId = root.icloudUser(text())
        onLoadFailed: root.calendarAppleId = ""
    }

    // The user of the first [[account]] with kind = "icloud". calendars.toml is written by the
    // settings page in a fixed shape (one key per line), which is all this reads.
    function icloudUser(toml) {
        const blocks = String(toml).split(/^\s*\[\[account\]\]\s*$/m).slice(1);
        for (const block of blocks) {
            const own = block.split(/^\s*\[\[/m)[0];
            if (!/^\s*kind\s*=\s*"icloud"\s*$/m.test(own)) continue;
            const m = own.match(/^\s*user\s*=\s*"([^"]*)"\s*$/m);
            if (m && m[1]) return m[1];
        }
        return "";
    }

    // The sign-in terminal writes this file when icloudpd is done; a sync then shows whether
    // it worked. (Syncing while it waits for the code would only send more codes.)
    FileView {
        path: `${root.cookieDir}/.signed-in`
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            if (!root.signInPending) return;
            root.signInPending = false;
            root.sync();
        }
    }

    function load() {} // for forcing initialisation

    IpcHandler {
        target: "icloudPhotos"

        function next(): void {
            root.next();
        }
        function sync(): void {
            root.sync();
        }
        function status(): string {
            return JSON.stringify({ status: root.status, message: root.message, album: root.album, count: root.count,
                appleId: root.appleId, active: root.active });
        }
    }
}
