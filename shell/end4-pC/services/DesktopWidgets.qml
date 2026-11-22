pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

// Which widgets sit on the desktop, and where. Placement is state, not
// config: it lives in $XDG_STATE_HOME/.../user/desktop-widgets.json and is
// written whenever a widget is dropped or added. Widgets snap to an 8px grid
// and stay where they are dropped.
Singleton {
    id: root

    readonly property int grid: 8

    // key: what the loader and the settings card call it. file: the widget's
    // window form under background/widgets/. free: its free form (no window,
    // straight on the background), free/<Name>Free.qml by the naming rule; a
    // free file that is not there yet, or fails to load, falls back to file.
    // Settings keeps reading key, icon and name.
    readonly property var catalog: [
        { key: "clock",      icon: "schedule",       name: "Display clock",   file: "desk/DisplayClockWidget.qml", free: "free/DisplayClockFree.qml" },
        { key: "vgaClock",   icon: "calendar_today", name: "Date",            file: "desk/VgaClockWidget.qml", free: "free/VgaClockFree.qml" },
        { key: "sheet",      icon: "person",         name: "Character sheet", file: "desk/CharacterSheetWidget.qml", free: "free/CharacterSheetFree.qml" },
        { key: "sky",        icon: "dark_mode",      name: "Sky",             file: "desk/SkyWidget.qml", free: "free/SkyFree.qml" },
        { key: "countdown",  icon: "event",          name: "Next event",      file: "desk/CountdownWidget.qml", free: "free/CountdownFree.qml" },
        { key: "todo",       icon: "add_task",       name: "To-do",           file: "desk/TodoListWidget.qml", free: "free/TodoListFree.qml" },
        { key: "timer",      icon: "timer",          name: "Focus timer",     file: "desk/FocusTimerWidget.qml", free: "free/FocusTimerFree.qml" },
        { key: "note",       icon: "note_stack_add", name: "Note",            file: "desk/NoteWidget.qml", free: "free/NoteFree.qml" },
        { key: "narrator",   icon: "chat",           name: "Narrator",        file: "desk/NarratorWidget.qml", free: "free/NarratorFree.qml" },
        { key: "power",      icon: "battery_full",   name: "Power",           file: "desk/PowerWidget.qml", free: "free/PowerFree.qml" },
        // system widgets (W1-widgets-b): widgets/system/<name>.qml
        { key: "weather",    icon: "partly_cloudy_day", name: "Weather",      file: "system/HourlyWeatherWidget.qml", free: "free/HourlyWeatherFree.qml" },
        { key: "nowPlaying", icon: "music_note",     name: "Now playing",     file: "system/NowPlayingWidget.qml", free: "free/NowPlayingFree.qml" },
        { key: "meters",     icon: "monitor_heart",  name: "Meters",          file: "system/MetersWidget.qml", free: "free/MetersFree.qml" },
        { key: "network",    icon: "network_check",  name: "Network",         file: "system/NetworkWidget.qml", free: "free/NetworkFree.qml" },
        { key: "disks",      icon: "hard_drive",     name: "Disks",           file: "system/DisksWidget.qml", free: "free/DisksFree.qml" },
        { key: "repos",      icon: "source",         name: "Repos",           file: "system/ReposWidget.qml", free: "free/ReposFree.qml" },
        { key: "updates",    icon: "system_update",  name: "Updates",         file: "system/UpdatesWidget.qml", free: "free/UpdatesFree.qml" },
        // Apple (widgets/Apple*.qml): AirPods through librepods, iCloud Drive, Cider
        { key: "airpods",    icon: "earbuds",        name: "AirPods",         file: "AppleAirPodsWidget.qml", free: "free/AppleAirPodsFree.qml" },
        { key: "icloudDrive", icon: "cloud",         name: "iCloud Drive",    file: "AppleDriveWidget.qml", free: "free/AppleDriveFree.qml" },
        { key: "lyrics",     icon: "lyrics",         name: "Lyrics",          file: "AppleLyricsWidget.qml", free: "free/AppleLyricsFree.qml" },
        { key: "queue",      icon: "queue_music",    name: "Up next",         file: "AppleQueueWidget.qml", free: "free/AppleQueueFree.qml" },
        // Claude (widgets/Claude*.qml): only while Claude is switched on (claude.enable).
        // Each runs only on a press and shows what it sends.
        { key: "claudeUsage",    icon: "data_usage",  name: "Claude usage",    file: "ClaudeUsageWidget.qml", free: "free/ClaudeUsageFree.qml",       needs: "claude" },
        { key: "claudeSessions", icon: "terminal",    name: "Claude sessions", file: "ClaudeSessionsWidget.qml", free: "free/ClaudeSessionsFree.qml",    needs: "claude" },
        { key: "claudeBrief",    icon: "wb_twilight", name: "Brief",           file: "ClaudeBriefWidget.qml", free: "free/ClaudeBriefFree.qml",       needs: "claude" },
        { key: "claudeUpdates",  icon: "update",      name: "What changed",    file: "ClaudeUpdateNotesWidget.qml", free: "free/ClaudeUpdateNotesFree.qml", needs: "claude" },
        { key: "claudeDigest",   icon: "inbox",       name: "Digest",          file: "ClaudeDigestWidget.qml", free: "free/ClaudeDigestFree.qml",      needs: "claude" },
        { key: "claudeSettings", icon: "tune",        name: "Ask about a setting", file: "ClaudeSettingsWidget.qml", free: "free/ClaudeSettingsFree.qml", needs: "claude" },
        { key: "claudeClipboard", icon: "content_paste", name: "Explain the clipboard", file: "ClaudeClipboardWidget.qml", free: "free/ClaudeClipboardFree.qml", needs: "claude" }
    ]
    // what this machine can show: the catalog less what needs something that is off
    readonly property var available: catalog.filter(w => !w.needs || root.has(w.needs))
    function has(need) {
        return need === "claude" ? (Config.options?.claude?.enable ?? false) : true;
    }

    // the Desktop-with-widgets board: who and when on the left, what is going
    // on on the right, the clock and the narrator at the bottom centre
    readonly property var defaults: ({
        sheet:      { anchor: "left",   y: 104 },
        sky:        { anchor: "left",   y: 344 },
        countdown:  { anchor: "left",   y: 504 },
        nowPlaying: { anchor: "right",  y: 104 },
        todo:       { anchor: "right",  y: 264 },
        timer:      { anchor: "right",  y: 456 },
        clock:      { anchor: "bottom", y: -372 },
        narrator:   { anchor: "bottom", y: -112 }
    })
    readonly property var defaultEnabled: ["sheet", "sky", "countdown", "nowPlaying", "todo", "timer", "clock", "narrator"]

    // The FreeDesktop board (1920x1080): whoami, sky and the clock with its
    // narration on the left; weather, countdown, network and meters in the
    // middle; todo, timer, now playing and repos on the right. Each edge is a
    // fraction of the screen (left/right of the width, top/bottom of the
    // height), measured from the side it names, so 1920x1080 is the board and
    // other sizes keep the columns.
    readonly property var freeDefaults: ({
        sheet:      { left: 96 / 1920,   top: 140 / 1080 },
        sky:        { left: 96 / 1920,   top: 360 / 1080 },
        clock:      { left: 96 / 1920,   bottom: 96 / 1080 },
        weather:    { left: 760 / 1920,  top: 130 / 1080 },
        countdown:  { left: 1100 / 1920, top: 150 / 1080 },
        network:    { left: 760 / 1920,  top: 560 / 1080 },
        meters:     { left: 760 / 1920,  bottom: 110 / 1080 },
        timer:      { left: 1150 / 1920, top: 520 / 1080 },
        todo:       { right: 96 / 1920,  top: 140 / 1080 },
        nowPlaying: { right: 96 / 1920,  top: 470 / 1080 },
        repos:      { right: 96 / 1920,  bottom: 110 / 1080 }
    })
    // the display clock carries the narration line in the free form, so the
    // narrator is not on by default there
    readonly property var freeDefaultEnabled: Object.keys(freeDefaults)

    // The widget style: "free" (the default) or "window", from the settings.
    // One widget can differ: st.forms[key], set from its label's right-click.
    readonly property string form: (Config.options?.background?.widgets?.form ?? "free") === "window" ? "window" : "free"
    function formOf(key) {
        const f = st.forms?.[key];
        return f === "free" || f === "window" ? f : root.form;
    }
    // Pins one widget to a form; the style the desktop already uses clears the pin.
    function setForm(key, f) {
        const next = Object.assign({}, st);
        const forms = Object.assign({}, st.forms ?? {});
        if (f === root.form) delete forms[key];
        else forms[key] = f;
        next.forms = forms;
        save(next);
    }
    // whether the free file is on disk (checked once at start, see freeProbe)
    function hasFree(key) {
        const w = catalog.find(c => c.key === key);
        return !!w?.free && root.freeFiles.includes(w.free);
    }
    // What the loader loads for a widget, relative to background/widgets/.
    // free: true picks the free file when there is one; the window file otherwise.
    function sourceOf(key, free) {
        const w = catalog.find(c => c.key === key);
        if (!w) return "";
        return free && root.hasFree(key) ? w.free : w.file;
    }

    readonly property var hidden: st.hidden ?? []
    readonly property var menuItems: available.filter(w => !hidden.includes(w.key))
    readonly property int enabledCount: available.filter(w => isEnabled(w.key)).length

    // saved state: { enabled: {key: bool}, pos: {key: {x, y, z}}, forms: {key: "free"|"window"},
    //                hidden: [], profile: {race, klass} }
    property var st: ({})
    property bool loaded: false

    // the sheet's two lines of flavour; nothing else about you is invented
    readonly property string race: st.profile?.race ?? "human"
    readonly property string klass: st.profile?.klass ?? "nix tinkerer"
    function setProfile(race, klass) {
        const next = Object.assign({}, st);
        next.profile = { race: race, klass: klass };
        save(next);
    }

    // The next calendar event, for the countdown. The calendar service writes
    // next-event.json ({ "title": "...", "start": "<ISO 8601 or epoch ms>" })
    // beside the state file; nothing here talks to a calendar itself.
    property var nextEvent: null
    readonly property string statePath: FileUtils.trimFileProtocol(`${Directories.state}/user/desktop-widgets.json`)
    readonly property string nextEventPath: FileUtils.trimFileProtocol(`${Directories.state}/user/next-event.json`)
    readonly property string notePath: FileUtils.trimFileProtocol(`${Directories.home}/notes/scratch.md`)

    // level of the character sheet: the system version
    property string systemVersion: ""

    function isEnabled(key) {
        const w = catalog.find(c => c.key === key);
        if (w?.needs && !root.has(w.needs)) return false;
        return st.enabled?.[key] ?? (root.form === "free" ? freeDefaultEnabled : defaultEnabled).includes(key);
    }

    function setEnabled(key, value) {
        const next = Object.assign({}, st);
        next.enabled = Object.assign({}, st.enabled ?? {}, { [key]: value });
        save(next);
    }

    function toggle(key) { setEnabled(key, !isEnabled(key)); }

    function snap(v) { return Math.round(v / grid) * grid; }

    // where a widget of size w x h goes on a screen of sw x sh. A saved
    // position wins in either form; without one, a free widget takes its
    // place on the FreeDesktop board and a window the Desktop-with-widgets one.
    function position(key, sw, sh, w, h) {
        const p = st.pos?.[key];
        const f = formOf(key) === "free" ? freeDefaults[key] : null;
        let x, y;
        if (p) {
            x = p.x; y = p.y;
        } else if (f) {
            x = f.right !== undefined ? sw - f.right * sw - w : (f.left ?? 0) * sw;
            y = f.bottom !== undefined ? sh - f.bottom * sh - h : (f.top ?? 0) * sh;
        } else {
            const d = defaults[key];
            if (!d) {
                // not on the board: the middle, each one a step along from the last
                const n = catalog.findIndex(c => c.key === key);
                x = (sw - w) / 2 + n * 16 - 128; y = (sh - h) / 2 + n * 16 - 128;
            } else if (d.anchor === "left") {
                x = 80; y = d.y;
            } else if (d.anchor === "right") {
                x = sw - 80 - w; y = d.y;
            } else {
                x = (sw - w) / 2; y = sh + d.y;
            }
        }
        return Qt.point(Math.max(0, Math.min(snap(x), Math.max(0, sw - w))), Math.max(0, Math.min(snap(y), Math.max(0, sh - h))));
    }

    // The one monitor the widgets are on, or "" when they are on every monitor.
    // A chosen monitor that is not plugged in falls back to the first one.
    readonly property string screenName: {
        const w = Config.options?.background?.widgets;
        if ((w?.screens ?? "all") !== "one") return "";
        const names = Quickshell.screens.map(s => s.name);
        return names.includes(w.monitor) ? w.monitor : (names[0] ?? "");
    }
    function showsOn(screen) {
        return root.screenName === "" || screen?.name === root.screenName;
    }

    function zOf(key) { return st.pos?.[key]?.z ?? 0; }

    function setPosition(key, x, y, z) {
        const next = Object.assign({}, st);
        next.pos = Object.assign({}, st.pos ?? {}, { [key]: { x: snap(x), y: snap(y), z: z ?? zOf(key) } });
        save(next);
    }

    function isStarred(key) { return !hidden.includes(key); }
    function setStarred(key, starred) {
        const next = Object.assign({}, st);
        const list = hidden.filter(k => k !== key);
        if (!starred) list.push(key);
        next.hidden = list;
        save(next);
    }
    function toggleStar(key) { setStarred(key, !isStarred(key)); }

    function save(next) {
        root.st = next;
        saveTimer.restart();
    }

    Timer {
        id: saveTimer
        interval: 400
        onTriggered: stateFile.setText(JSON.stringify(root.st, null, 2))
    }

    FileView {
        id: stateFile
        path: root.statePath
        onLoaded: {
            try {
                root.st = JSON.parse(stateFile.text());
            } catch (e) {
                console.warn("[DesktopWidgets] state file is not JSON, starting over:", e);
                root.st = {};
            }
            root.loaded = true;
        }
        onLoadFailed: error => {
            if (error !== FileViewError.FileNotFound)
                console.warn("[DesktopWidgets] could not read the state file:", error);
            root.loaded = true;
        }
    }

    // Which free files exist, relative to background/widgets/. Until a widget's
    // free form lands it has none, and it shows as a window without a warning.
    property var freeFiles: []
    property bool freeChecked: false
    readonly property string widgetsDir: FileUtils.trimFileProtocol(Qt.resolvedUrl("../modules/ii/background/widgets/").toString())
    Process {
        id: freeProbe
        running: true
        command: ["sh", "-c", 'cd "$1" && shift && for f; do [ -f "$f" ] && printf "%s\\n" "$f"; done; true', "sh", root.widgetsDir]
            .concat(root.catalog.filter(c => c.free).map(c => c.free))
        stdout: StdioCollector {
            onStreamFinished: root.freeFiles = this.text.split("\n").filter(l => l.length > 0)
        }
        onExited: root.freeChecked = true
    }

    FileView {
        id: nextEventFile
        path: root.nextEventPath
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const o = JSON.parse(nextEventFile.text());
                const t = typeof o.start === "number" ? o.start : Date.parse(o.start);
                root.nextEvent = isNaN(t) ? null : { title: String(o.title ?? ""), start: t, source: o.source ?? "calendar" };
            } catch (e) {
                root.nextEvent = null;
            }
        }
        onLoadFailed: root.nextEvent = null
    }

    FileView {
        id: osRelease
        path: "/etc/os-release"
        onLoaded: root.systemVersion = (osRelease.text().match(/^VERSION_ID="?([^"\n]+)"?/m) ?? [])[1] ?? ""
    }
}
