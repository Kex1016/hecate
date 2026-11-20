pragma Singleton
pragma ComponentBehavior: Bound
import QtCore
import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.modules.ii.settings

// The calendar service: syncs every account in ~/.config/hecate/calendars.toml
// (iCloud and other CalDAV servers, read-only .ics subscriptions, and the local
// calendar) and keeps the events for the dashboard, the quick panel, the
// countdown widget and whoever else asks.
//
// The work is done by scripts/apple/dav.py, which reads each account's
// app-specific password from the keyring itself (secret-tool, attributes
// `application hecate service calendars account <id>`), so no password passes
// through QML, a command line, a config file or a log. It writes:
//   ~/.local/state/quickshell/user/calendars/events.json   (the shown calendars,
//       recurrences expanded from 3 months back to 13 months ahead)
//       [{ title, start, end, allDay, account, accountId, calendar, href,
//          uid, url, etag, color, location, notes, recurring, writable }]
//   ~/.local/state/quickshell/user/contacts/contacts.json  (see Contacts.qml)
// and prints one JSON line per account, which goes to CalendarAccounts' status.
// This service writes user/next-event.json ({ title, start, ... }) for the
// countdown widget.
//
// Times: `start`/`end` are ISO 8601. Timed events carry an offset; all-day
// events are local midnight with no offset ("2026-11-20T00:00:00"), so
// `new Date(start)` lands on the right day everywhere. An all-day `end` is
// exclusive, as in iCalendar.
//
//   Calendars.eventDays(2026, 10)        → [3, 4, 21]   (month is 0-11)
//   Calendars.eventsOn(new Date())       → today's events, by start
//   Calendars.nextEvent                  → the next event to start, or null
//   Calendars.createEvent({ title: "dentist", start: d, end: d2 }, (error, ev) => ...)
Singleton {
    id: root

    // ── what others read ──────────────────────────────────────────────────────────

    // Every synced event of the shown calendars, sorted by start.
    property var events: []
    // events.json has been read (it may be empty).
    property bool ready: false
    property bool syncing: false
    // ISO time of the last sync that finished, and its error ("" when every account synced).
    property string lastSynced: ""
    property string error: ""

    // The next event to start (not one already under way), or null.
    property var nextEvent: null

    // Where createEvent() puts an event when none is named: Config's
    // calendars.newEventsTo, "local" or "<account name>: <calendar name>".
    readonly property string defaultTarget: Config.options?.calendars?.newEventsTo ?? "local"
    // Every place an event can go, in the same form.
    readonly property var targets: CalendarAccounts.newEventTargets

    readonly property string stateDir: FileUtils.trimFileProtocol(`${Directories.state}/user/calendars`)
    readonly property string eventsPath: `${root.stateDir}/events.json`
    readonly property string nextEventPath: FileUtils.trimFileProtocol(`${Directories.state}/user/next-event.json`)
    readonly property string contactsPath: FileUtils.trimFileProtocol(`${Directories.state}/user/contacts/contacts.json`)
    // The local calendar, a plain .ics file of its own.
    readonly property string localPath: FileUtils.trimFileProtocol(`${StandardPaths.standardLocations(StandardPaths.GenericDataLocation)[0]}/hecate/calendars/local.ics`)
    readonly property string script: FileUtils.trimFileProtocol(`${Directories.scriptPath}/apple/dav.py`)

    // ── queries ───────────────────────────────────────────────────────────────────

    // The events touching a day (a Date, or anything new Date() takes).
    function eventsOn(day) {
        const d = new Date(day);
        const from = new Date(d.getFullYear(), d.getMonth(), d.getDate());
        const to = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1);
        return root.eventsBetween(from, to);
    }

    // The events overlapping [from, to).
    function eventsBetween(from, to) {
        const a = new Date(from).getTime();
        const b = new Date(to).getTime();
        return root.events.filter(e => root.startMs(e) < b && root.endMs(e) > a);
    }

    // The events of a month (month 0-11), multi-day ones included.
    function eventsIn(year, month) {
        return root.eventsBetween(new Date(year, month, 1), new Date(year, month + 1, 1));
    }

    // The days (1-31) of a month that have at least one event, for CalendarGrid's
    // eventDays. A multi-day event marks every day it touches.
    function eventDays(year, month) {
        const first = new Date(year, month, 1).getTime();
        const last = new Date(year, month + 1, 0).getDate();
        const days = {};
        for (const e of root.eventsIn(year, month)) {
            const s = Math.max(root.startMs(e), first);
            const end = root.endMs(e);
            for (let d = new Date(s); d.getTime() < end && d.getMonth() === month; d = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1)) {
                days[d.getDate()] = true;
                if (d.getDate() >= last) break;
            }
        }
        return Object.keys(days).map(n => parseInt(n)).sort((x, y) => x - y);
    }

    // The next `count` events to start after `after` (default now).
    function upcoming(count, after) {
        const t = after ? new Date(after).getTime() : Date.now();
        return root.events.filter(e => root.startMs(e) > t).slice(0, count ?? 5);
    }

    // The events whose title, location or notes contain `text` (any case), soonest first
    // from today on, then the past ones, latest first.
    function search(text, count) {
        const q = String(text ?? "").toLowerCase().trim();
        if (q === "") return [];
        const hit = root.events.filter(e => [e.title, e.location, e.notes].some(s => String(s ?? "").toLowerCase().includes(q)));
        const n = new Date();
        const today = new Date(n.getFullYear(), n.getMonth(), n.getDate()).getTime();
        const ahead = hit.filter(e => root.endMs(e) > today);
        const past = hit.filter(e => root.endMs(e) <= today).reverse();
        return ahead.concat(past).slice(0, count ?? 20);
    }

    function startMs(e) {
        return new Date(e.start).getTime();
    }
    function endMs(e) {
        const end = e.end ? new Date(e.end).getTime() : NaN;
        const start = root.startMs(e);
        if (!isNaN(end) && end > start) return end;
        // No (or an empty) duration: a day for all-day events, else a moment.
        return start + (e.allDay ? 86400000 : 1);
    }

    // ── changes ───────────────────────────────────────────────────────────────────

    // Adds an event and syncs. ev: { title, start, end, allDay, location, notes,
    // calendar } where start/end are Dates or ISO strings (end defaults to an hour
    // after start, or the next day when allDay) and calendar is a target (see
    // `targets`, default `defaultTarget`). done(error, event): error is "" on success,
    // event the stored one (with uid and url).
    function createEvent(ev, done) {
        const start = new Date(ev.start);
        if (isNaN(start.getTime())) {
            done?.("The event has no start time.", null);
            return;
        }
        const allDay = !!ev.allDay;
        let end = ev.end ? new Date(ev.end) : null;
        if (!end || isNaN(end.getTime()) || end <= start)
            end = allDay ? new Date(start.getFullYear(), start.getMonth(), start.getDate() + 1) : new Date(start.getTime() + 3600000);
        const request = {
            title: String(ev.title ?? "").trim() || "event",
            start: allDay ? root.localDate(start) : start.toISOString(),
            end: allDay ? root.localDate(end) : end.toISOString(),
            allDay,
            location: String(ev.location ?? ""),
            notes: String(ev.notes ?? ""),
            target: ev.calendar || root.defaultTarget,
            tz: root.timeZone
        };
        root.change(["create"], request, done);
    }

    // Removes an event (one from `events`; for a repeating event, the whole series).
    // done(error).
    function deleteEvent(ev, done) {
        if (!ev || !ev.uid) {
            done?.("That is not a calendar event.");
            return;
        }
        if (!ev.writable) {
            done?.("That calendar is read-only.");
            return;
        }
        root.change(["delete"], { uid: ev.uid, url: ev.url ?? "", etag: ev.etag ?? "", accountId: ev.accountId, href: ev.href }, (error) => done?.(error));
    }

    // Syncs now (a sync already running is followed by one more).
    function sync() {
        if (root.syncing) {
            root.again = true;
            return;
        }
        root.syncing = true;
        root.again = false;
        root.runScript(["sync"], null, line => root.onSyncLine(line), (exit, lastError) => {
            root.syncing = false;
            root.lastSynced = new Date().toISOString();
            root.error = exit === 0 ? root.collectedError : (lastError || `The calendar sync stopped (exit ${exit}).`);
            root.collectedError = "";
            eventsFile.reload();
            if (root.again) root.sync();
        });
    }

    // Starts the service; called once from shell.qml.
    function load() {
        if (root.started) return;
        root.startedAt = Date.now();
        root.started = true;
    }

    // ── internals ─────────────────────────────────────────────────────────────────

    property bool started: false
    property bool again: false
    property string collectedError: ""
    readonly property string timeZone: {
        // Olson name of the local zone, for the TZID of new timed events.
        try {
            return Intl.DateTimeFormat().resolvedOptions().timeZone ?? "";
        } catch (e) {
            return "";
        }
    }

    function localDate(d) {
        const p = n => String(n).padStart(2, "0");
        return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
    }

    function onSyncLine(o) {
        if (o.account === undefined) return;
        const patch = { state: o.state, message: o.message ?? "" };
        if (o.state === "ok") {
            patch.synced = o.synced ?? new Date().toISOString();
            patch.checked = patch.synced;
        } else if (o.message) {
            root.collectedError = root.collectedError || o.message;
        }
        CalendarAccounts.setStatus(o.account, patch);
    }

    function change(args, request, done) {
        let result = null;
        root.runScript(args, request, line => {
            if (line.ok !== undefined) result = line;
            else root.onSyncLine(line);
        }, (exit, lastError) => {
            eventsFile.reload();
            if (result?.ok) done?.("", result.event ?? null);
            else done?.(result?.error || lastError || `The calendar would not take it (exit ${exit}).`, null);
        });
    }

    // Runs dav.py with the paths it needs; `input` (an object) goes on stdin as JSON.
    // Each JSON line it prints goes to onLine; done(exit, last error line).
    function runScript(args, input, onLine, done) {
        const command = ["sh", "-c", 'command -v python3 >/dev/null 2>&1 || exit 127; exec python3 "$@"', "sh", root.script].concat(args).concat([
            "--config", Directories.calendarsPath,
            "--state", root.stateDir,
            "--local", root.localPath,
            "--contacts", root.contactsPath
        ]);
        const job = jobComponent.createObject(root, { command, input: input ? JSON.stringify(input) + "\n" : "", onLine, done });
        job.running = true;
    }

    Component {
        id: jobComponent
        Process {
            id: job
            property string input: ""
            property var onLine: null
            property var done: null
            property string lastError: ""
            property int exit: -1
            property int pending: 2
            stdinEnabled: true
            stdout: SplitParser {
                onRead: data => {
                    try {
                        job.onLine?.(JSON.parse(data));
                    } catch (e) {
                        // not ours
                    }
                }
            }
            stderr: SplitParser {
                onRead: data => {
                    if (data.trim() !== "") job.lastError = data.trim();
                }
            }
            onRunningChanged: {
                if (!running) return;
                if (job.input !== "") job.write(job.input);
                job.stdinEnabled = false;
            }
            onExited: code => {
                if (code === 127 && job.lastError === "") job.lastError = "python3 is missing, so calendars cannot sync.";
                job.done?.(code, job.lastError);
                job.destroy();
            }
        }
    }

    function updateNext() {
        const next = root.upcoming(1)[0] ?? null;
        const key = next ? `${next.uid}|${next.start}|${next.title}` : "";
        if (key === root.nextKey) return;
        root.nextKey = key;
        root.nextEvent = next;
        nextFile.setText(JSON.stringify(next ? {
            title: next.title,
            start: next.start,
            end: next.end,
            allDay: next.allDay,
            account: next.account,
            calendar: next.calendar,
            location: next.location ?? "",
            source: "calendar"
        } : {}) + "\n");
    }
    property string nextKey: "-"

    onEventsChanged: root.updateNext()

    // once a minute: the next event moves on as the clock passes it
    Timer {
        running: root.started
        interval: 60000
        repeat: true
        onTriggered: root.updateNext()
    }

    Timer {
        id: syncTimer
        running: root.started && CalendarAccounts.loaded
        interval: Math.max(1, Config.options?.calendars?.syncInterval ?? 15) * 60000
        repeat: true
        triggeredOnStart: true
        onTriggered: root.sync()
    }

    // An account added or changed, or a calendar shown or hidden: sync soon.
    Timer {
        id: accountsChanged
        interval: 1500
        onTriggered: root.sync()
    }
    Connections {
        target: CalendarAccounts
        enabled: root.started
        function onAccountsChanged() {
            if (CalendarAccounts.loaded) accountsChanged.restart();
        }
        function onStatusChanged() {
            // `r` on the calendars page; an older request was before this shell
            const asked = CalendarAccounts.status?.syncRequested ?? "";
            if (asked === "" || asked === root.lastRequest || Date.parse(asked) < root.startedAt) return;
            root.lastRequest = asked;
            accountsChanged.restart();
        }
    }
    property string lastRequest: ""
    property real startedAt: 0

    FileView {
        id: eventsFile
        path: root.eventsPath
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                const list = JSON.parse(eventsFile.text());
                root.events = Array.isArray(list) ? list.slice().sort((a, b) => root.startMs(a) - root.startMs(b)) : [];
            } catch (e) {
                root.events = [];
            }
            root.ready = true;
        }
        onLoadFailed: root.ready = true
    }

    FileView {
        id: nextFile
        path: root.nextEventPath
        printErrors: false
    }
}
