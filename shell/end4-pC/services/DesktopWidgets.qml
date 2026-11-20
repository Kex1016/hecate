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
    // QML under background/widgets/. w: the width it asks for, so the default
    // columns line up. Settings keeps reading key, icon and name.
    readonly property var catalog: [
        { key: "clock",      icon: "schedule",       name: "Display clock",   file: "desk/DisplayClockWidget.qml" },
        { key: "vgaClock",   icon: "calendar_today", name: "Date",            file: "desk/VgaClockWidget.qml" },
        { key: "sheet",      icon: "person",         name: "Character sheet", file: "desk/CharacterSheetWidget.qml" },
        { key: "sky",        icon: "dark_mode",      name: "Sky",             file: "desk/SkyWidget.qml" },
        { key: "countdown",  icon: "event",          name: "Next event",      file: "desk/CountdownWidget.qml" },
        { key: "todo",       icon: "add_task",       name: "To-do",           file: "desk/TodoListWidget.qml" },
        { key: "timer",      icon: "timer",          name: "Focus timer",     file: "desk/FocusTimerWidget.qml" },
        { key: "note",       icon: "note_stack_add", name: "Note",            file: "desk/NoteWidget.qml" },
        { key: "narrator",   icon: "chat",           name: "Narrator",        file: "desk/NarratorWidget.qml" },
        { key: "power",      icon: "battery_full",   name: "Power",           file: "desk/PowerWidget.qml" },
        // system widgets (W1-widgets-b): widgets/system/<name>.qml
        { key: "weather",    icon: "partly_cloudy_day", name: "Weather",      file: "system/HourlyWeatherWidget.qml" },
        { key: "nowPlaying", icon: "music_note",     name: "Now playing",     file: "system/NowPlayingWidget.qml" },
        { key: "meters",     icon: "monitor_heart",  name: "Meters",          file: "system/MetersWidget.qml" },
        { key: "network",    icon: "network_check",  name: "Network",         file: "system/NetworkWidget.qml" },
        { key: "disks",      icon: "hard_drive",     name: "Disks",           file: "system/DisksWidget.qml" },
        { key: "repos",      icon: "source",         name: "Repos",           file: "system/ReposWidget.qml" },
        { key: "updates",    icon: "system_update",  name: "Updates",         file: "system/UpdatesWidget.qml" },
        // Apple (widgets/Apple*.qml): AirPods through librepods, iCloud Drive, Cider
        { key: "airpods",    icon: "earbuds",        name: "AirPods",         file: "AppleAirPodsWidget.qml" },
        { key: "icloudDrive", icon: "cloud",         name: "iCloud Drive",    file: "AppleDriveWidget.qml" },
        { key: "lyrics",     icon: "lyrics",         name: "Lyrics",          file: "AppleLyricsWidget.qml" },
        { key: "queue",      icon: "queue_music",    name: "Up next",         file: "AppleQueueWidget.qml" },
        // Claude (widgets/Claude*.qml): only while Claude is switched on (claude.enable).
        // Each runs only on a press and shows what it sends.
        { key: "claudeUsage",    icon: "data_usage",  name: "Claude usage",    file: "ClaudeUsageWidget.qml",       needs: "claude" },
        { key: "claudeSessions", icon: "terminal",    name: "Claude sessions", file: "ClaudeSessionsWidget.qml",    needs: "claude" },
        { key: "claudeBrief",    icon: "wb_twilight", name: "Brief",           file: "ClaudeBriefWidget.qml",       needs: "claude" },
        { key: "claudeUpdates",  icon: "update",      name: "What changed",    file: "ClaudeUpdateNotesWidget.qml", needs: "claude" },
        { key: "claudeDigest",   icon: "inbox",       name: "Digest",          file: "ClaudeDigestWidget.qml",      needs: "claude" },
        { key: "claudeSettings", icon: "tune",        name: "Ask about a setting", file: "ClaudeSettingsWidget.qml", needs: "claude" },
        { key: "claudeClipboard", icon: "content_paste", name: "Explain the clipboard", file: "ClaudeClipboardWidget.qml", needs: "claude" }
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

    readonly property var hidden: st.hidden ?? []
    readonly property var menuItems: available.filter(w => !hidden.includes(w.key))
    readonly property int enabledCount: available.filter(w => isEnabled(w.key)).length

    // saved state: { enabled: {key: bool}, pos: {key: {x, y, z}}, hidden: [], profile: {race, klass} }
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
        return st.enabled?.[key] ?? defaultEnabled.includes(key);
    }

    function setEnabled(key, value) {
        const next = Object.assign({}, st);
        next.enabled = Object.assign({}, st.enabled ?? {}, { [key]: value });
        save(next);
    }

    function toggle(key) { setEnabled(key, !isEnabled(key)); }

    function snap(v) { return Math.round(v / grid) * grid; }

    // where a widget of size w x h goes on a screen of sw x sh
    function position(key, sw, sh, w, h) {
        const p = st.pos?.[key];
        let x, y;
        if (p) {
            x = p.x; y = p.y;
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
