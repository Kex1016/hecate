import qs
import qs.modules.common
import qs.modules.common.models
import qs.modules.common.functions
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Io
pragma Singleton
pragma ComponentBehavior: Bound

/**
 * The wallpaper picker's folder browser, thumbnails, preview and apply (wallpaper mode only;
 * the default desktop is the compositor's blobs).
 */
Singleton {
    id: root

    property string thumbgenScriptPath: `${FileUtils.trimFileProtocol(Directories.scriptPath)}/thumbnails/thumbgen-venv.sh`
    property string generateThumbnailsMagickScriptPath: `${FileUtils.trimFileProtocol(Directories.scriptPath)}/thumbnails/generate-thumbnails-magick.sh`
    function getCleanDirPath(path) {
        if (!path) return "";
        return FileUtils.trimFileProtocol(path.toString()).replace(/\/+$/, "");
    }

    property alias directory: folderModel.folder
    readonly property string effectiveDirectory: getCleanDirPath(folderModel.folder)
    property url defaultFolder: Qt.resolvedUrl(`${Directories.pictures}/Wallpapers`)
    property alias folderModel: folderModel // Expose for direct binding when needed
    property alias wallpaperModel: wallpaperModel
    property string sortMode: Config.options.wallpaperSelector?.sortMode || "custom"
    onSortModeChanged: debounceFilterTimer.restart()
    property var orderMap: ({})
    property bool orderLoaded: false
    property string searchQuery: ""
    readonly property list<string> extensions: [ // TODO: add videos
        "jpg", "jpeg", "png", "webp", "avif", "bmp", "svg"
    ]
    property list<string> wallpapers: [] // List of absolute file paths (without file://)
    readonly property bool thumbnailGenerationRunning: thumbgenProc.running
    property real thumbnailGenerationProgress: 0
    property string previewPath: ""  // Set during arrow navigation; empty by default
    property string confirmedPath: ""  // Holds confirmed path until config catches up

    signal changed()
    signal thumbnailGenerated(directory: string)
    signal thumbnailGeneratedFile(filePath: string)

    function load () {} // For forcing initialization

    function isVideo(path) {
        return /\.(mp4|webm|mkv|avi|mov)$/i.test(path ?? "");
    }

    // A monitor's own wallpaper, or "" when it shows the main one.
    function monitorWallpaper(name) {
        return Config.options.background.monitorWallpapers.find(e => e.monitor === name)?.path ?? "";
    }

    // Give a monitor its own wallpaper; "" puts it back on the main one.
    function setMonitorWallpaper(name, path) {
        const rest = Config.options.background.monitorWallpapers.filter(e => e.monitor !== name);
        Config.options.background.monitorWallpapers = path ? rest.concat([{ monitor: name, path: path }]) : rest;
    }

    function startPreview(path) {
        if (!path || path.length === 0) return;
        root.previewPath = path;
    }

    function stopPreview() {
        root.previewPath = "";
    }


    // Wallpapers are opt-in: the default desktop is the compositor's animated blobs.
    readonly property bool enabled: Config.options.background.mode === "wallpaper"
    readonly property bool dither: Config.options.background.dither
    readonly property bool perMonitor: Config.options.background.perMonitor

    function setMode(mode) {
        Config.options.background.mode = mode === "wallpaper" ? "wallpaper" : "blobs";
        if (mode !== "wallpaper") GlobalStates.wallpaperSelectorOpen = false;
    }

    function setDither(on) {
        Config.options.background.dither = on;
    }

    function setPerMonitor(on) {
        Config.options.background.perMonitor = on;
        if (!on) Config.options.background.monitorWallpapers = [];
    }

    // ── dithering ────────────────────────────────────────────────────────────────
    // With `background.dither` on, the background shows a copy of the picture pressed into
    // two tones of the night sky (base and muted): fitted to the screen like the original,
    // turned grey and stretched to the full range (like the album art), ordered-dithered with
    // the 4x4 Bayer matrix at one cell per ditherCell pixels and scaled back up with
    // nearest-neighbour. The copies are PNGs in ditherDir,
    // named after the picture, its mtime, the size and the tones, so a copy is made once and
    // a changed picture gets a new one. ImageMagick does the work in a child process, one
    // picture at a time, never on the UI thread. The picker's previews come from the same
    // call at their own size.

    readonly property string ditherDir: FileUtils.trimFileProtocol(`${Directories.cache}/wallpapers/dithered`)
    readonly property color ditherDark: Appearance.haiiro.color.base
    readonly property color ditherLight: Appearance.haiiro.color.muted
    // Screen pixels per dither cell: half the design system's 4px checker tile, so a
    // dithered wallpaper has the same grain as the placeholders and the album art.
    readonly property int ditherCell: Math.max(1, Appearance.haiiro.size.ditherTile / 2)
    // "<path>|<w>x<h>" -> the dithered file, or "!" when it could not be made.
    property var ditherCache: ({})
    // Picked up by the queue, not bound to anything.
    property var ditherPending: []

    function ditherKey(path, width, height) {
        return `${FileUtils.trimFileProtocol(String(path))}|${Math.round(width)}x${Math.round(height)}`;
    }

    // The dithered copy of `path` at width x height (device pixels), as a path, or "" while it
    // is being made (or when it can't be: `ditherFailed`). Safe to call from a binding: it
    // reads ditherCache, so the binding updates when the copy is ready.
    function ditheredPath(path, width, height) {
        const clean = FileUtils.trimFileProtocol(String(path ?? ""));
        if (clean.length === 0 || !(width > 0) || !(height > 0)) return "";
        const key = root.ditherKey(clean, width, height);
        const hit = root.ditherCache[key];
        if (hit === "!") return "";
        if (hit) return hit;
        root.requestDither(clean, Math.round(width), Math.round(height), false);
        return "";
    }

    // The same, for a screen: its copy is made before any waiting preview.
    function ditheredScreenPath(path, width, height) {
        const clean = FileUtils.trimFileProtocol(String(path ?? ""));
        if (clean.length === 0 || !(width > 0) || !(height > 0)) return "";
        const hit = root.ditherCache[root.ditherKey(clean, width, height)];
        if (hit === "!") return "";
        if (hit) return hit;
        root.requestDither(clean, Math.round(width), Math.round(height), true);
        return "";
    }

    function ditherFailed(path, width, height) {
        return root.ditherCache[root.ditherKey(path, width, height)] === "!";
    }

    // What a background or preview should show for `path`: the dithered copy when dithering
    // is on (or "" until it is ready), else the picture itself.
    function displayPath(path, width, height) {
        if (!root.dither) return FileUtils.trimFileProtocol(String(path ?? ""));
        return root.ditheredPath(path, width, height);
    }

    function requestDither(path, width, height, urgent) {
        const key = root.ditherKey(path, width, height);
        if (ditherProc.key === key) return;
        const queued = root.ditherPending.findIndex(j => j.key === key);
        if (queued >= 0 && !urgent) return;
        if (queued >= 0) root.ditherPending.splice(queued, 1);
        const job = { key: key, path: path, width: width, height: height };
        // The screens' own sizes go first; previews wait.
        if (urgent) root.ditherPending.unshift(job);
        else root.ditherPending.push(job);
        ditherQueueTimer.restart();
    }

    Timer {
        id: ditherQueueTimer
        interval: 0
        onTriggered: root.nextDither()
    }

    function nextDither() {
        if (ditherProc.running || ditherProc.key.length > 0 || root.ditherPending.length === 0) return;
        const job = root.ditherPending.shift();
        ditherProc.key = job.key;
        ditherProc.command = ["sh", "-c", root.ditherScript, "dither", job.path, String(job.width), String(job.height),
            String(root.ditherCell), String(root.ditherDark), String(root.ditherLight), root.ditherDir];
        ditherProc.running = true;
    }

    // $1 picture, $2 width, $3 height, $4 cell, $5 dark, $6 light, $7 cache dir.
    // Prints "ok <file>" or "error <why>". Keeps the 48 newest copies.
    readonly property string ditherScript: [
        'src=$1; w=$2; h=$3; cell=$4; dark=$5; light=$6; dir=$7',
        'mt=$(stat -c %Y "$src" 2>/dev/null) || { echo "error missing"; exit 1; }',
        'key=$(printf "%s|%s|%sx%s|%s|%s|%s|1" "$src" "$mt" "$w" "$h" "$cell" "$dark" "$light" | sha1sum | cut -c1-20)',
        'out="$dir/$key.png"',
        'if [ -s "$out" ]; then touch "$out"; echo "ok $out"; exit 0; fi',
        'mkdir -p "$dir" || { echo "error mkdir"; exit 1; }',
        'sw=$(( (w + cell - 1) / cell )); sh=$(( (h + cell - 1) / cell ))',
        'tmp="$dir/.$key.$$.png"',
        'if magick "$src[0]" -auto-orient -resize "${sw}x${sh}^" -gravity center -extent "${sw}x${sh}" \\',
        '    -colorspace Gray -auto-level -ordered-dither o4x4 -clamp \\',
        '    +level-colors "$dark,$light" -scale "$((cell * 100))%" -extent "${w}x${h}" \\',
        '    "PNG8:$tmp" 2>/dev/null; then mv -f "$tmp" "$out"; else rm -f "$tmp"; echo "error magick"; exit 1; fi',
        'ls -1t "$dir"/*.png 2>/dev/null | tail -n +49 | while read -r f; do rm -f "$f"; done',
        'echo "ok $out"',
    ].join("\n")

    Process {
        id: ditherProc
        property string key: ""
        // The script always prints one line; it is the whole answer.
        stdout: StdioCollector {
            onStreamFinished: ditherProc.finish(text.trim())
        }
        function finish(line) {
            if (ditherProc.key.length === 0) return;
            const next = Object.assign({}, root.ditherCache);
            if (line.startsWith("ok ")) {
                next[ditherProc.key] = line.slice(3);
            } else {
                console.warn(`[Wallpapers] could not dither ${ditherProc.key.split("|")[0]}: ${line || "no answer"}`);
                next[ditherProc.key] = "!";
            }
            root.ditherCache = next;
            ditherProc.key = "";
            ditherQueueTimer.restart();
        }
        // A process that never started has no output to finish it.
        onRunningChanged: if (!running) ditherWatchdog.restart()
    }

    Timer {
        id: ditherWatchdog
        interval: 1000
        onTriggered: if (ditherProc.key.length > 0 && !ditherProc.running) ditherProc.finish("")
    }

    // No recolouring: the palette never follows the picture. The background layer
    // reads wallpaperPath and, with dither on, shows ditheredScreenPath() of it.
    function apply(path, darkMode = Appearance.m3colors.darkmode) {
        if (!path || path.length === 0) return;
        root.confirmedPath = path;
        Config.options.background.wallpaperPath = path;
        root.changed()
    }

    function openFolder() {
        Quickshell.execDetached(["xdg-open", root.effectiveDirectory]);
    }

    Process {
        id: selectProc
        property string filePath: ""
        property bool darkMode: Appearance.m3colors.darkmode
        property var onFileSelected: null
        function select(filePath, darkMode = Appearance.m3colors.darkmode, onFileSelected = null) {
            selectProc.filePath = filePath
            selectProc.darkMode = darkMode
            selectProc.onFileSelected = onFileSelected
            selectProc.exec(["test", "-d", FileUtils.trimFileProtocol(filePath)])
        }
        onExited: (exitCode, exitStatus) => {
            if (exitCode === 0) {
                setDirectory(selectProc.filePath);
                return;
            }
            if (selectProc.onFileSelected) {
                selectProc.onFileSelected(selectProc.filePath);
            } else {
                root.apply(selectProc.filePath, selectProc.darkMode);
            }
        }
    }

    function select(filePath, darkMode = Appearance.m3colors.darkmode, onFileSelected = null) {
        selectProc.select(filePath, darkMode, onFileSelected);
    }

    function randomFromCurrentFolder(darkMode = Appearance.m3colors.darkmode) {
        if (!root.enabled) return;
        const count = wallpaperModel.count > 0 ? wallpaperModel.count : folderModel.count;
        if (count === 0) return;
        const randomIndex = Math.floor(Math.random() * count);
        const item = wallpaperModel.count > 0 ? wallpaperModel.get(randomIndex) : null;
        const filePath = item ? item.filePath : folderModel.get(randomIndex, "filePath");
        print("Randomly selected wallpaper:", filePath);
        if (filePath) root.select(filePath, darkMode);
    }

    function getRandomWallpaperPath(excludePath = "") {
        const count = wallpaperModel.count > 0 ? wallpaperModel.count : folderModel.count;
        if (count === 0) return "";
        const excludeClean = FileUtils.trimFileProtocol(excludePath);
        const candidates = [];
        for (let i = 0; i < count; i++) {
            const item = wallpaperModel.count > 0 ? wallpaperModel.get(i) : null;
            const path = item ? item.filePath : (folderModel.get(i, "filePath") || FileUtils.trimFileProtocol(folderModel.get(i, "fileURL")));
            if (path && path.length && FileUtils.trimFileProtocol(path) !== excludeClean) {
                candidates.push(path);
            }
        }
        if (candidates.length === 0) return "";
        return candidates[Math.floor(Math.random() * candidates.length)];
    }

    Process {
        id: validateDirProc
        property string nicePath: ""
        function setDirectoryIfValid(path) {
            validateDirProc.nicePath = FileUtils.trimFileProtocol(path).replace(/\/+$/, "")
            if (/^\/*$/.test(validateDirProc.nicePath)) validateDirProc.nicePath = "/";
            validateDirProc.exec([
                "bash", "-c",
                `if [ -d "${validateDirProc.nicePath}" ]; then echo dir; elif [ -f "${validateDirProc.nicePath}" ]; then echo file; else echo invalid; fi`
            ])
        }
        stdout: StdioCollector {
            onStreamFinished: {
                    root.directory = Qt.resolvedUrl(validateDirProc.nicePath)
                const result = text.trim()
                if (result === "dir") {
                } else if (result === "file") {
                    root.directory = Qt.resolvedUrl(FileUtils.parentDirectory(validateDirProc.nicePath))
                } else {
                    // Ignore
                }
            }
        }
    }
    function setDirectory(path) {
        validateDirProc.setDirectoryIfValid(path)
    }
    function navigateUp() {
        folderModel.navigateUp()
    }
    function navigateBack() {
        folderModel.navigateBack()
    }
    function navigateForward() {
        folderModel.navigateForward()
    }

    // Folder model
    FolderListModelWithHistory {
        id: folderModel
        folder: Qt.resolvedUrl(root.defaultFolder)
        caseSensitive: false
        nameFilters: root.extensions.map(ext => `*.${ext}`)
        showDirs: true
        showDotAndDotDot: false
        showOnlyReadable: true
        sortField: FolderListModel.Time
        sortReversed: false
        onCountChanged: debounceRebuildTimer.restart()
        onStatusChanged: {
            if (status === FolderListModel.Ready) debounceRebuildTimer.restart();
        }
    }

    onEffectiveDirectoryChanged: debounceRebuildTimer.restart()
    onSearchQueryChanged: debounceFilterTimer.restart()

    ListModel {
        id: wallpaperModel
    }

    FileView {
        id: orderFileView
        path: `${Directories.shellConfig}/wallpaper_order.json`
        watchChanges: false
        onLoaded: {
            try {
                const txt = orderFileView.text();
                if (txt && txt.trim().length > 0) {
                    root.orderMap = JSON.parse(txt);
                } else {
                    root.orderMap = {};
                }
            } catch (e) {
                console.log("[Wallpapers] Error parsing wallpaper_order.json:", e);
                root.orderMap = {};
            }
            root.orderLoaded = true;
            debounceRebuildTimer.restart();
        }
        onLoadFailed: (error) => {
            root.orderMap = {};
            root.orderLoaded = true;
            debounceRebuildTimer.restart();
        }
    }

    Connections {
        target: Config.options.wallpaperSelector ?? null
        function onSortModeChanged() {
            if (Config.options.wallpaperSelector?.sortMode && root.sortMode !== Config.options.wallpaperSelector.sortMode) {
                root.sortMode = Config.options.wallpaperSelector.sortMode;
                debounceFilterTimer.restart();
            }
        }
    }

    Connections {
        target: Config
        function onReadyChanged() {
            if (Config.ready) {
                if (Config.options.wallpaperSelector?.sortMode) {
                    root.sortMode = Config.options.wallpaperSelector.sortMode;
                }
                debounceRebuildTimer.restart();
            }
        }
    }

    function saveCustomOrder() {
        const jsonStr = JSON.stringify(root.orderMap, null, 2);
        if (orderFileView) {
            try {
                orderFileView.setText(jsonStr);
            } catch (e) {
                console.log("[Wallpapers] Failed to save wallpaper_order.json:", e);
            }
        }
        const filePath = `${Directories.shellConfig}/wallpaper_order.json`;
        Quickshell.execDetached(["bash", "-c", `mkdir -p '${Directories.shellConfig}' && cat << 'EOF' > '${filePath}.tmp' && mv '${filePath}.tmp' '${filePath}'\n${jsonStr}\nEOF`]);
    }

    function moveWallpaper(fromIndex, toIndex) {
        if (root.searchQuery.trim().length > 0) return;
        if (fromIndex < 0 || toIndex < 0 || fromIndex >= wallpaperModel.count || toIndex >= wallpaperModel.count || fromIndex === toIndex)
            return;

        wallpaperModel.move(fromIndex, toIndex, 1);
        root.sortMode = "custom";
        if (Config.options.wallpaperSelector) {
            Config.options.wallpaperSelector.sortMode = "custom";
        }
        Config.setNestedValue("wallpaperSelector.sortMode", "custom");

        const list = [];
        const paths = [];
        for (let i = 0; i < wallpaperModel.count; i++) {
            const it = wallpaperModel.get(i);
            list.push(it.fileName);
            if (it.filePath) paths.push(it.filePath);
        }
        const cleanDir = getCleanDirPath(folderModel.folder);
        root.orderMap[cleanDir] = list;
        root.wallpapers = paths;
        root.saveCustomOrder();
    }

    function moveToTop(index) {
        moveWallpaper(index, 0);
    }

    function moveToBottom(index) {
        moveWallpaper(index, wallpaperModel.count - 1);
    }

    function setSortMode(mode) {
        root.sortMode = mode;
        if (Config.options.wallpaperSelector) {
            Config.options.wallpaperSelector.sortMode = mode;
        }
        Config.setNestedValue("wallpaperSelector.sortMode", mode);
        applyFilter();
    }

    Timer {
        id: debounceRebuildTimer
        interval: 20
        repeat: false
        onTriggered: root.rebuildWallpaperModel()
    }

    Timer {
        id: debounceFilterTimer
        interval: 120
        repeat: false
        onTriggered: root.applyFilter()
    }

    function sortItems(items, mode, customList) {
        if (mode === "custom") {
            if (!customList || customList.length === 0) {
                return items.slice().sort((a, b) => {
                    if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                    return new Date(b.fileModified) - new Date(a.fileModified);
                });
            }
            const orderLookup = {};
            for (let i = 0; i < customList.length; i++) {
                orderLookup[customList[i]] = i;
            }
            const dirs = [];
            const orderedFiles = [];
            const remainingFiles = [];
            for (let i = 0; i < items.length; i++) {
                const it = items[i];
                if (it.fileIsDir) {
                    dirs.push(it);
                } else if (typeof orderLookup[it.fileName] !== "undefined") {
                    orderedFiles.push(it);
                } else {
                    remainingFiles.push(it);
                }
            }
            dirs.sort((a, b) => a.fileName.localeCompare(b.fileName, undefined, { numeric: true, sensitivity: "base" }));
            orderedFiles.sort((a, b) => orderLookup[a.fileName] - orderLookup[b.fileName]);
            remainingFiles.sort((a, b) => new Date(b.fileModified) - new Date(a.fileModified));
            return dirs.concat(orderedFiles, remainingFiles);
        } else if (mode === "name") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return a.fileName.localeCompare(b.fileName, undefined, { numeric: true, sensitivity: "base" });
            });
        } else if (mode === "name_rev") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return b.fileName.localeCompare(a.fileName, undefined, { numeric: true, sensitivity: "base" });
            });
        } else if (mode === "time") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return new Date(b.fileModified) - new Date(a.fileModified);
            });
        } else if (mode === "time_rev") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return new Date(a.fileModified) - new Date(b.fileModified);
            });
        } else if (mode === "size") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return (b.fileSize || 0) - (a.fileSize || 0);
            });
        } else if (mode === "size_rev") {
            return items.slice().sort((a, b) => {
                if (a.fileIsDir !== b.fileIsDir) return a.fileIsDir ? -1 : 1;
                return (a.fileSize || 0) - (b.fileSize || 0);
            });
        }
        return items;
    }

    property var allItems: []
    signal resultsUpdated()

    function normalizeText(text) {
        return String(text ?? "").toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "");
    }

    function searchKeyFor(name) {
        return root.normalizeText(name.replace(/\.[^.]+$/, "")).replace(/[_\-.]+/g, " ");
    }

    function scoreToken(key, token) {
        const index = key.indexOf(token);
        if (index >= 0) {
            const wordStart = index === 0 || key.charAt(index - 1) === " ";
            return 200 + (wordStart ? 60 : 0) - Math.min(index, 40) + Math.min(token.length, 12);
        }
        if (token.length < 3) return -1;
        let position = 0;
        let gaps = 0;
        let last = -1;
        for (let i = 0; i < token.length; i++) {
            const found = key.indexOf(token.charAt(i), position);
            if (found < 0) return -1;
            if (last >= 0) gaps += found - last - 1;
            last = found;
            position = found + 1;
        }
        return Math.max(1, 100 - gaps * 3 - token.length);
    }

    function scoreItem(key, tokens) {
        let total = 0;
        for (const token of tokens) {
            const score = root.scoreToken(key, token);
            if (score < 0) return -1;
            total += score;
        }
        return total;
    }

    function itemKey(item) {
        return item.filePath || item.fileName;
    }

    function syncModel(list) {
        const wanted = new Set(list.map(item => root.itemKey(item)));
        for (let i = wallpaperModel.count - 1; i >= 0; i--) {
            if (!wanted.has(root.itemKey(wallpaperModel.get(i)))) wallpaperModel.remove(i);
        }
        for (let target = 0; target < list.length; target++) {
            const item = list[target];
            const key = root.itemKey(item);
            if (target < wallpaperModel.count && root.itemKey(wallpaperModel.get(target)) === key) {
                const current = wallpaperModel.get(target);
                if (current.fileModified !== item.fileModified || current.fileSize !== item.fileSize) wallpaperModel.set(target, item);
                continue;
            }
            let found = -1;
            for (let j = target + 1; j < wallpaperModel.count; j++) {
                if (root.itemKey(wallpaperModel.get(j)) === key) {
                    found = j;
                    break;
                }
            }
            if (found >= 0) wallpaperModel.move(found, target, 1);
            else wallpaperModel.insert(target, item);
        }
    }

    function applyFilter() {
        const tokens = root.normalizeText(root.searchQuery).split(/\s+/).filter(token => token.length > 0);
        let sorted;
        if (tokens.length > 0) {
            const scored = [];
            for (const item of root.allItems) {
                const score = root.scoreItem(item.searchKey, tokens);
                if (score >= 0) scored.push({ item: item, score: score });
            }
            scored.sort((a, b) => b.score - a.score || a.item.fileName.localeCompare(b.item.fileName, undefined, { numeric: true, sensitivity: "base" }));
            sorted = scored.map(entry => entry.item);
        } else {
            const cleanDir = getCleanDirPath(folderModel.folder);
            const savedOrder = root.orderMap[cleanDir] || root.orderMap[cleanDir + "/"] || [];
            const effectiveMode = (root.sortMode === "custom" || (!root.sortMode && savedOrder.length > 0)) ? "custom" : root.sortMode;
            sorted = sortItems(root.allItems, effectiveMode, savedOrder);
        }

        root.syncModel(sorted);
        root.wallpapers = sorted.filter(item => item.filePath && item.filePath.length).map(item => item.filePath);
        root.resultsUpdated();
    }

    function rebuildWallpaperModel() {
        const count = folderModel.count;
        const items = [];
        for (let i = 0; i < count; i++) {
            const fn = folderModel.get(i, "fileName") || "";
            const fp = folderModel.get(i, "filePath") || "";
            const fu = (fp && fp.length) ? ("file://" + fp) : (folderModel.get(i, "fileUrl") || "");
            const modified = folderModel.get(i, "fileModified");
            items.push({
                fileName: fn,
                filePath: fp,
                fileUrl: fu,
                fileURL: fu,
                fileIsDir: Boolean(folderModel.get(i, "fileIsDir")),
                fileSize: folderModel.get(i, "fileSize") || 0,
                fileModified: modified ? modified.toString() : "",
                searchKey: root.searchKeyFor(fn)
            });
        }
        root.allItems = items;
        root.applyFilter();
    }

    // Thumbnail generation
    function generateThumbnail(size: string) {
        if (!["normal", "large", "x-large", "xx-large"].includes(size)) throw new Error("Invalid thumbnail size");
        thumbgenProc.directory = root.directory
        thumbgenProc.running = false
        thumbgenProc.command = [
            "bash", "-c",
            `${thumbgenScriptPath} --size ${size} --machine_progress -d ${FileUtils.trimFileProtocol(root.directory)} || ${generateThumbnailsMagickScriptPath} --size ${size} -d ${FileUtils.trimFileProtocol(root.directory)}`,
        ]
        // console.log("[Wallpapers] Updating thumbnails with command ", thumbgenProc.command.join(" "))
        root.thumbnailGenerationProgress = 0
        thumbgenProc.running = true
    }
    Process {
        id: thumbgenProc
        property string directory
        stdout: SplitParser {
            onRead: data => {
                // print("thumb gen proc:", data)
                let match = data.match(/PROGRESS (\d+)\/(\d+)/)
                if (match) {
                    const completed = parseInt(match[1])
                    const total = parseInt(match[2])
                    root.thumbnailGenerationProgress = completed / total
                }
                match = data.match(/FILE (.+)/)
                if (match) {
                    const filePath = match[1]
                    root.thumbnailGeneratedFile(filePath)
                }
            }
        }
        onExited: (exitCode, exitStatus) => {
            // print("[Wallpapers] Thumbnail generation completed with exit code", exitCode)
            root.thumbnailGenerated(thumbgenProc.directory)
        }
    }

    IpcHandler {
        target: "wallpapers"

        function apply(path: string): void {
            if (!root.enabled) return;
            root.apply(path);
        }

        function setSortMode(mode: string): void {
            root.setSortMode(mode);
        }

        function moveWallpaper(fromIndex: int, toIndex: int): void {
            root.moveWallpaper(fromIndex, toIndex);
        }
    }
}