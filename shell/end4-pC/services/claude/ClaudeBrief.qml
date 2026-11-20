pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions

// The hourly brief: a few lines about the evening written by `claude -p` from
// what the shell already knows (next event, weather, repos, updates).
//
// Nothing here runs by itself. A brief is written when the narrator widget's
// button is pressed, or on the hour once the user turned the schedule on; that
// switch is off until then and is remembered in claude-brief.json. The exact
// text handed to Claude is `context`, and the widget shows it.
//
// Also keeps the time of the last lock, so the notification digest knows what
// "while you were away" means. Touch `ClaudeBrief.load()` at shell start so
// that is tracked even when no widget is open.
Singleton {
    id: root

    readonly property string statePath: FileUtils.trimFileProtocol(`${Directories.state}/user/claude-brief.json`)
    readonly property string binary: Quickshell.env("HECATE_CLAUDE_BIN") || "claude"
    readonly property string toolPath: `${Quickshell.env("PATH")}:${Quickshell.env("HOME")}/.local/bin:/run/current-system/sw/bin:/etc/profiles/per-user/${Quickshell.env("USER")}/bin:${Quickshell.env("HOME")}/.nix-profile/bin`

    // saved: { enabled: bool, everyMinutes: int, text: string, writtenAt: ms }
    property bool scheduled: false
    property int everyMinutes: 60
    property string text: ""
    property double writtenAt: 0
    property string error: ""
    readonly property bool running: proc.running
    readonly property double nextAt: scheduled && writtenAt > 0 ? writtenAt + everyMinutes * 60000 : 0

    // time of the last lock, for the digest (0 = never locked in this session)
    property double lockedAt: 0
    property bool loaded: false

    function load() {}

    function stamp(ms) {
        return ms > 0 ? Qt.formatDateTime(new Date(ms), "HH:mm") : "";
    }

    // what is sent, as the lines the widget lists; `context` is the same thing as one string
    readonly property var contextLines: {
        const l = [];
        const now = new Date();
        l.push("now: " + Qt.formatDateTime(now, "dddd HH:mm"));
        const w = Weather.data;
        if (w && w.description)
            l.push(`weather: ${w.description}, ${w.temp ?? ""}`.trim());
        const ev = DesktopWidgets.nextEvent;
        if (ev)
            l.push(`next event: ${ev.title} at ${Qt.formatDateTime(new Date(ev.start), "ddd HH:mm")}`);
        if (Repos.available) {
            for (const r of Repos.repos.filter(r => r.state !== "clean").slice(0, 6))
                l.push(`repo ${r.name}: ${r.state}`);
        }
        if (Updates.cakeos && Updates.cakeosStatus && Updates.cakeosStatus.behind > 0)
            l.push(`cakeos updates waiting: ${Updates.cakeosStatus.behind}`);
        else if (Updates.available && !Updates.cakeos && Updates.count > 0)
            l.push(`package updates waiting: ${Updates.count}`);
        return l;
    }
    readonly property string context: contextLines.join("\n")

    readonly property string systemPrompt: "You write a short brief for someone's desktop: two or three plain sentences, no markdown, no greeting, no lists. Mention only what the facts below say; invent nothing. Plain words, calm tone."

    function generate() {
        if (proc.running) return;
        root.error = "";
        proc.command = [root.binary, "-p", "--tools", "", "--no-session-persistence", "--output-format", "text", "--append-system-prompt", root.systemPrompt];
        proc.running = true;
    }

    function setScheduled(on) {
        root.scheduled = on;
        root.save();
    }

    function save() {
        stateFile.setText(JSON.stringify({ enabled: root.scheduled, everyMinutes: root.everyMinutes, text: root.text, writtenAt: root.writtenAt }, null, 2));
    }

    Process {
        id: proc
        stdinEnabled: true
        environment: ({ PATH: root.toolPath })
        onStarted: {
            write(root.context + "\n");
            stdinEnabled = false;
        }
        stdout: StdioCollector {
            onStreamFinished: {
                const out = text.trim();
                if (out.length > 0) {
                    root.text = out;
                    root.writtenAt = Date.now();
                    root.save();
                }
            }
        }
        stderr: StdioCollector {
            onStreamFinished: if (text.trim().length > 0) root.error = text.trim().split("\n")[0]
        }
        onExited: (code, status) => {
            stdinEnabled = true;
            if (code !== 0 && root.error === "") root.error = "claude exited with " + code;
        }
    }

    // opt-in schedule: checked every minute, runs only when it is on and a brief is due
    Timer {
        interval: 60000
        repeat: true
        running: root.loaded && root.scheduled
        onTriggered: {
            if (!proc.running && Date.now() >= root.nextAt && !GlobalStates.screenLocked)
                root.generate();
        }
    }

    Connections {
        target: GlobalStates
        function onScreenLockedChanged() {
            if (GlobalStates.screenLocked) root.lockedAt = Date.now();
        }
    }

    FileView {
        id: stateFile
        path: root.statePath
        onLoaded: {
            try {
                const o = JSON.parse(stateFile.text());
                root.scheduled = o.enabled === true;
                root.everyMinutes = Math.max(15, Number(o.everyMinutes) || 60);
                root.text = String(o.text ?? "");
                root.writtenAt = Number(o.writtenAt) || 0;
            } catch (e) {}
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
    }
}
