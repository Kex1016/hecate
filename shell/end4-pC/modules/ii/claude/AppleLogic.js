.pragma library

// Pure logic behind the Apple* Claude cards: which prompt each card sends,
// how a reply is read back, and what the card shows before anything is
// created or played. No QML types here so `node AppleLogic.test.mjs` can
// check it.
//
// Times are local wall-clock strings "YYYY-MM-DDTHH:MM" (all-day events
// "YYYY-MM-DD"), the same form the cards show and edit.

var DAYS = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
var MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

function pad(n) { return (n < 10 ? "0" : "") + n; }

function dateStr(d) { return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()); }
function timeStr(d) { return pad(d.getHours()) + ":" + pad(d.getMinutes()); }
function stamp(d) { return dateStr(d) + "T" + timeStr(d); }

// "2026-11-21T20:00" or "2026-11-21" or epoch ms or Date -> Date | null
function parseTime(v) {
    if (v === null || v === undefined || v === "") return null;
    if (v instanceof Date) return isNaN(v.getTime()) ? null : v;
    if (typeof v === "number") return new Date(v);
    var m = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{1,2}):(\d{2})(?::(\d{2}))?)?(Z|[+-]\d{2}:?\d{2})?$/.exec(String(v).trim());
    if (!m) return null;
    var d;
    if (m[7]) {
        d = new Date(String(v).trim().replace(" ", "T"));
        return isNaN(d.getTime()) ? null : d;
    }
    d = new Date(+m[1], +m[2] - 1, +m[3], m[4] ? +m[4] : 0, m[5] ? +m[5] : 0, m[6] ? +m[6] : 0);
    return isNaN(d.getTime()) ? null : d;
}

function isAllDay(v) { return typeof v === "string" && /^\d{4}-\d{2}-\d{2}$/.test(v.trim()); }

// "sat 21 nov · 20:00–22:00"
function whenLine(ev) {
    var s = parseTime(ev.start), e = parseTime(ev.end);
    if (!s) return "";
    var day = DAYS[s.getDay()] + " " + s.getDate() + " " + MONTHS[s.getMonth()];
    if (ev.allDay || isAllDay(ev.start)) return day + " · all day";
    if (!e) return day + " · " + timeStr(s);
    if (dateStr(s) !== dateStr(e)) {
        return day + " " + timeStr(s) + " → " + DAYS[e.getDay()] + " " + e.getDate() + " " + MONTHS[e.getMonth()] + " " + timeStr(e);
    }
    return day + " · " + timeStr(s) + "–" + timeStr(e);
}

// ---- reading the reply -------------------------------------------------

// The Claude Code CLI with --output-format json prints one object whose
// `result` is the model's text; a fake or older CLI may print the text bare.
function replyText(stdout) {
    var t = String(stdout || "").trim();
    if (t === "") return { ok: false, error: "claude printed nothing" };
    var o = null;
    try { o = JSON.parse(t); } catch (e) { o = null; }
    if (o && typeof o === "object" && !Array.isArray(o) && ("result" in o || "is_error" in o)) {
        if (o.is_error) return { ok: false, error: String(o.result || o.subtype || "claude reported an error") };
        return { ok: true, text: String(o.result === undefined ? "" : o.result).trim() };
    }
    return { ok: true, text: t };
}

// First JSON object (or array) in model text, fenced or not.
function extractJson(text) {
    var t = String(text || "");
    var fence = /```(?:json)?\s*([\s\S]*?)```/i.exec(t);
    if (fence) t = fence[1];
    var start = -1, open = "";
    for (var i = 0; i < t.length; i++) {
        if (t[i] === "{" || t[i] === "[") { start = i; open = t[i]; break; }
    }
    if (start < 0) return null;
    var close = open === "{" ? "}" : "]";
    var depth = 0, inStr = false, esc = false;
    for (var j = start; j < t.length; j++) {
        var c = t[j];
        if (inStr) {
            if (esc) esc = false;
            else if (c === "\\") esc = true;
            else if (c === "\"") inStr = false;
            continue;
        }
        if (c === "\"") inStr = true;
        else if (c === open) depth++;
        else if (c === close) {
            depth--;
            if (depth === 0) {
                try { return JSON.parse(t.slice(start, j + 1)); } catch (e) { return null; }
            }
        }
    }
    return null;
}

// ---- which words go where ---------------------------------------------

// The launcher's `?` hands us whatever was typed. This decides whether one of
// the Apple cards owns it ("event", "free", "queue") or the ask panel does
// ("other"). Only a hint: the user still presses enter before anything is sent.
function intent(text) {
    var t = String(text || "").toLowerCase().trim();
    if (t === "") return "other";
    if (/\b(when|what time)\b.*\b(free|available|open)\b|\bam i (free|busy)\b|\bfree (time|slot|this|next|on|after)\b|\bany(thing)? (free|on)\b/.test(t)) return "free";
    if (/^(play|queue|put on|shuffle)\b|\b(playlist|queue)\b.*\b(for|of|with)\b|\bplay (me )?(something|some|a |an )/.test(t)) return "queue";
    var when = /\b(mon|tue|wed|thu|fri|sat|sun)[a-z]*\b|\b(today|tonight|tomorrow|next week)\b|\b\d{1,2}(:\d{2})?\s?(am|pm)\b|\b\d{1,2}:\d{2}\b|\bat \d{1,2}\b|\b(sat|sun|mon|tue|wed|thu|fri)\s+\d{1,2}\b/.test(t);
    var what = /\b(dinner|lunch|breakfast|brunch|meeting|call|meet|appointment|party|visit|trip|dentist|doctor|birthday|gym|class|stream|session|night|date|coffee)\b/.test(t);
    if (when && (what || /\bwith\b/.test(t))) return "event";
    return "other";
}

// ---- calendar access, against whatever the Calendars service exposes ---

// Where an event can go. The Calendars service lists `targets` as strings:
// "local" or "<account name>: <calendar name>". Objects { id, name, account,
// writable } are taken too.
function calendarList(cal) {
    if (!cal) return [];
    var list = cal.targets || cal.calendars;
    if (!list || !list.length) return [];
    var out = [];
    for (var i = 0; i < list.length; i++) {
        var c = list[i];
        if (typeof c === "string") {
            var at = c.indexOf(": ");
            out.push(at > 0 ? { id: c, name: c.slice(at + 2), account: c.slice(0, at), writable: true } : { id: c, name: c, account: "", writable: true });
        } else {
            out.push({ id: String(c.id !== undefined ? c.id : c.name), name: String(c.name !== undefined ? c.name : c.id), account: c.account ? String(c.account) : "", writable: c.writable !== false });
        }
    }
    return out;
}

// The calendars whose events the service holds (it keeps only the shown
// ones): "account: calendar", one per calendar that has an event.
function shownCalendars(events) {
    var seen = {}, out = [];
    for (var i = 0; i < events.length; i++) {
        var e = events[i], key = (e.account ? e.account + ": " : "") + e.calendar;
        if (!e.calendar || seen[key]) continue;
        seen[key] = true;
        out.push({ id: key, name: e.calendar, account: e.account || "" });
    }
    return out;
}

function eventList(cal) {
    var list = cal && cal.events;
    if (!list || !list.length) return [];
    var out = [];
    for (var i = 0; i < list.length; i++) {
        var e = list[i];
        var s = parseTime(e.start), en = parseTime(e.end);
        if (!s) continue;
        out.push({
            uid: e.uid !== undefined ? String(e.uid) : "",
            title: String(e.title || e.summary || "untitled"),
            start: s, end: en || s,
            allDay: !!e.allDay || isAllDay(e.start),
            calendar: String(e.calendar || e.calendarName || ""),
            account: e.account ? String(e.account) : "",
            location: e.location ? String(e.location) : "",
            description: e.description ? String(e.description) : (e.notes ? String(e.notes) : ""),
            attendees: e.attendees || [],
            attachments: e.attachments || []
        });
    }
    out.sort(function (a, b) { return a.start - b.start; });
    return out;
}

// Monday 00:00 of the week `d` is in, and the Monday after.
function weekRange(d) {
    var start = new Date(d.getFullYear(), d.getMonth(), d.getDate() - ((d.getDay() + 6) % 7));
    var end = new Date(start.getFullYear(), start.getMonth(), start.getDate() + 7);
    return { start: start, end: end };
}

// ---- event card ---------------------------------------------------------

function eventPrompt(text, now, calendars) {
    var names = calendars.map(function (c) { return c.id; });
    return [
        "You turn a few plain words into one calendar event. Reply with ONLY a JSON object, no prose, no code fence:",
        "{\"title\": string, \"start\": \"YYYY-MM-DDTHH:MM\", \"end\": \"YYYY-MM-DDTHH:MM\", \"allDay\": boolean, \"calendar\": string, \"location\": string, \"invite\": [string]}",
        "Rules: resolve days like \"sat\" or \"tomorrow\" against the current time below. If no end is given make it 2 hours after the start for a meal or night out, otherwise 1 hour. Put the other person's name in `invite` only if the words say with whom. Pick `calendar` from this list only: " + JSON.stringify(names) + ". Use \"\" for anything not stated. Do not invent details.",
        "Current local time: " + stamp(now) + " (" + DAYS[now.getDay()] + ").",
        "Words: " + JSON.stringify(String(text))
    ].join("\n");
}

// What the card may show and later create. Returns { ok, event } or { ok:false, error }.
function normalizeEvent(raw, now, calendars, defaultCalendar) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return { ok: false, error: "claude's reply was not an event" };
    var title = String(raw.title || "").trim();
    if (title === "") return { ok: false, error: "no title in the reply" };
    var s = parseTime(raw.start);
    if (!s) return { ok: false, error: "no usable start time in the reply" };
    var allDay = !!raw.allDay || isAllDay(raw.start);
    var e = parseTime(raw.end);
    if (allDay) {
        s = new Date(s.getFullYear(), s.getMonth(), s.getDate());
        e = e && e > s ? new Date(e.getFullYear(), e.getMonth(), e.getDate()) : new Date(s.getFullYear(), s.getMonth(), s.getDate() + 1);
    } else if (!e || e <= s) {
        e = new Date(s.getTime() + 60 * 60 * 1000);
    }
    var cals = calendars || [];
    var cal = null;
    var want = String(raw.calendar || "").toLowerCase();
    for (var i = 0; i < cals.length; i++) {
        if (cals[i].name.toLowerCase() === want || cals[i].id.toLowerCase() === want) { cal = cals[i]; break; }
    }
    if (!cal && defaultCalendar) {
        for (var k = 0; k < cals.length; k++) if (cals[k].id === defaultCalendar || cals[k].name === defaultCalendar) { cal = cals[k]; break; }
    }
    if (!cal && cals.length) {
        for (var j = 0; j < cals.length; j++) if (cals[j].writable !== false) { cal = cals[j]; break; }
    }
    var invite = [];
    if (Array.isArray(raw.invite)) {
        for (var n = 0; n < raw.invite.length; n++) {
            var who = String(raw.invite[n] || "").trim();
            if (who) invite.push(who);
        }
    }
    return {
        ok: true,
        event: {
            title: title,
            start: allDay ? dateStr(s) : stamp(s),
            end: allDay ? dateStr(e) : stamp(e),
            allDay: allDay,
            calendar: cal ? cal.id : "",
            calendarName: cal ? cal.name : String(raw.calendar || ""),
            account: cal ? cal.account : "",
            location: String(raw.location || "").trim(),
            invite: invite,
            past: !allDay && s < now
        }
    };
}

// The lines the card shows: title / when / where it goes.
function eventCardLines(ev) {
    var where = (ev.account ? ev.account + ": " : "") + (ev.calendarName || "no calendar");
    var third = where + (ev.invite && ev.invite.length ? " · with " + ev.invite.join(", ") : "");
    var lines = [ev.title, whenLine(ev)];
    if (ev.location) lines.push("at " + ev.location);
    lines.push(third);
    return lines;
}

// An edit field's text back into the event; null when it does not parse.
function applyEdit(ev, field, value) {
    var out = {};
    for (var k in ev) out[k] = ev[k];
    var v = String(value).trim();
    if (field === "title") {
        if (v === "") return null;
        out.title = v;
    } else if (field === "start" || field === "end") {
        var d = parseTime(v);
        if (!d) return null;
        out[field] = out.allDay ? dateStr(d) : stamp(d);
        var s = parseTime(out.start), e = parseTime(out.end);
        if (field === "start" && e && e <= s) out.end = out.allDay ? dateStr(new Date(s.getFullYear(), s.getMonth(), s.getDate() + 1)) : stamp(new Date(s.getTime() + 60 * 60 * 1000));
        if (field === "end" && e <= s) return null;
    } else if (field === "location") {
        out.location = v;
    } else if (field === "invite") {
        out.invite = v === "" ? [] : v.split(",").map(function (x) { return x.trim(); }).filter(function (x) { return x !== ""; });
    } else {
        return null;
    }
    return out;
}

// ---- free time ----------------------------------------------------------

// Window of events sent along: the current week plus the next one, so
// "next week" and "this weekend" both have data.
function freeWindow(now) {
    var w = weekRange(now);
    return { start: new Date(now.getFullYear(), now.getMonth(), now.getDate()), end: new Date(w.end.getFullYear(), w.end.getMonth(), w.end.getDate() + 7) };
}

function eventsIn(events, win) {
    return events.filter(function (e) { return e.end > win.start && e.start < win.end; });
}

function eventLine(e) {
    var when = e.allDay ? dateStr(e.start) + " all day" : stamp(e.start) + "–" + (dateStr(e.start) === dateStr(e.end) ? timeStr(e.end) : stamp(e.end));
    return when + " " + e.title + (e.calendar ? " [" + e.calendar + "]" : "");
}

// Only title, time and calendar name leave the machine, never descriptions,
// attendees or locations.
function freeContext(events, now) {
    var win = freeWindow(now);
    var inWin = eventsIn(events, win);
    var shown = shownCalendars(events);
    return {
        window: win,
        events: inWin,
        calendarNames: shown.map(function (c) { return c.id; }),
        summary: "reading " + (shown.length ? shown.map(function (c) { return c.id; }).join(", ") : "no calendars") + " · " + inWin.length + " events · " + dateStr(win.start).slice(5) + " to " + dateStr(new Date(win.end.getTime() - 1)).slice(5)
    };
}

function freePrompt(question, ctx, now) {
    return [
        "Answer a question about when the user is free, in plain words, two or three short sentences, no lists, no markdown. Use only the events below; the user is free whenever nothing is listed. Name days, not dates, where you can. Say which events get in the way.",
        "Current local time: " + stamp(now) + " (" + DAYS[now.getDay()] + "). Awake hours are 08:00 to 23:00.",
        "Events (local time, [calendar]):",
        ctx.events.length ? ctx.events.map(eventLine).join("\n") : "(none)",
        "Question: " + JSON.stringify(String(question))
    ].join("\n");
}

// ---- the note before an event ------------------------------------------

var NOTE_TEXT_LIMIT = 6000;

// The note card shows exactly this list before sending anything.
function noteContext(ev) {
    var parts = [];
    parts.push({ label: "invite", text: ev.description || "" });
    var atts = ev.attachments || [];
    for (var i = 0; i < atts.length; i++) {
        var a = atts[i];
        parts.push({ label: typeof a === "string" ? a : String(a.name || "attachment"), text: typeof a === "string" ? "" : String(a.text || "") });
    }
    return parts;
}

function noteSummary(ev) {
    var parts = noteContext(ev);
    var items = [];
    if (parts[0].text) items.push("invite");
    var files = 0, unread = 0;
    for (var i = 1; i < parts.length; i++) {
        if (parts[i].text) files++;
        else unread++;
    }
    if (files) items.push(files + (files === 1 ? " attachment" : " attachments"));
    var s = items.length ? "reading " + items.join(" + ") : "nothing to read";
    if (unread) s += " · " + unread + " not readable";
    return s;
}

function noteHasContent(ev) {
    var parts = noteContext(ev);
    for (var i = 0; i < parts.length; i++) if (parts[i].text) return true;
    return false;
}

function notePrompt(ev) {
    var parts = noteContext(ev), budget = NOTE_TEXT_LIMIT, body = [];
    for (var i = 0; i < parts.length && budget > 0; i++) {
        if (!parts[i].text) continue;
        var t = parts[i].text.slice(0, budget);
        budget -= t.length;
        body.push("--- " + parts[i].label + " ---\n" + t);
    }
    return [
        "Write ONE or at most two short sentences for a notification shown just before this event. Say only what the invite and attachments below tell the user to bring, do, or discuss first. If they say nothing useful, reply with exactly: NOTHING. Plain text, no markdown, no greeting.",
        "Event: " + ev.title + (ev.calendar ? " [" + ev.calendar + "]" : "") + " at " + whenLine({ start: stamp(ev.start), end: stamp(ev.end), allDay: ev.allDay }),
        body.join("\n")
    ].join("\n");
}

function cleanNote(text) {
    var t = String(text || "").replace(/[`*_#>]/g, "").replace(/\s+/g, " ").trim();
    if (t === "" || /^nothing\.?$/i.test(t)) return "";
    return t.length > 280 ? t.slice(0, 277) + "..." : t;
}

// ---- the music queue ----------------------------------------------------

var LIBRARY_LIMIT = 120;

function libraryContext(lib) {
    var list = (lib && lib.length) ? lib : [];
    var out = [];
    for (var i = 0; i < list.length && out.length < LIBRARY_LIMIT; i++) {
        var t = list[i];
        out.push({ id: t.id !== undefined ? String(t.id) : "", title: String(t.title || t.name || ""), artist: String(t.artist || ""), album: String(t.album || ""), seconds: Number(t.seconds || t.duration || 0) });
    }
    return out;
}

function queueSummary(libCtx, total) {
    var shown = libCtx.length;
    return "sending " + shown + (total > shown ? " of " + total : "") + " library titles";
}

function queuePrompt(request, libCtx) {
    return [
        "Build a music queue from the user's own Apple Music library for the request below. Reply with ONLY a JSON object, no prose, no code fence:",
        "{\"name\": string, \"tracks\": [{\"id\": string, \"title\": string, \"artist\": string}]}",
        "Rules: choose only tracks from the library list (copy `id` exactly, never invent tracks). 8 to 30 tracks, in the order they should play. `name` is a short playlist name.",
        "Library (id | title | artist | album | seconds):",
        libCtx.map(function (t) { return [t.id, t.title, t.artist, t.album, t.seconds].join(" | "); }).join("\n"),
        "Request: " + JSON.stringify(String(request))
    ].join("\n");
}

// Keeps only tracks that really are in the library; Claude cannot add songs.
function normalizeQueue(raw, libCtx) {
    if (!raw || typeof raw !== "object") return { ok: false, error: "claude's reply was not a queue" };
    var arr = Array.isArray(raw) ? raw : raw.tracks;
    if (!Array.isArray(arr)) return { ok: false, error: "no tracks in the reply" };
    var byId = {}, byKey = {};
    for (var i = 0; i < libCtx.length; i++) {
        var t = libCtx[i];
        if (t.id) byId[t.id] = t;
        byKey[(t.title + "|" + t.artist).toLowerCase()] = t;
    }
    var seen = {}, tracks = [], dropped = 0;
    for (var n = 0; n < arr.length; n++) {
        var r = arr[n] || {};
        var hit = (r.id && byId[String(r.id)]) || byKey[(String(r.title || "") + "|" + String(r.artist || "")).toLowerCase()];
        if (!hit) { dropped++; continue; }
        var key = hit.id || (hit.title + "|" + hit.artist);
        if (seen[key]) continue;
        seen[key] = true;
        tracks.push(hit);
    }
    if (!tracks.length) return { ok: false, error: "none of the suggested tracks are in your library" };
    var seconds = 0;
    for (var k = 0; k < tracks.length; k++) seconds += tracks[k].seconds || 0;
    return { ok: true, queue: { name: String(raw.name || "").trim() || "queue from claude", tracks: tracks, seconds: seconds, dropped: dropped } };
}

function duration(seconds) {
    var m = Math.round(seconds / 60);
    if (m < 60) return m + "m";
    return Math.floor(m / 60) + "h " + pad(m % 60) + "m";
}

function trackLine(t) { return t.artist ? t.artist + " · " + t.title : t.title; }

// What the Calendars service's createEvent() takes: Dates, not strings (an
// all-day "2026-11-21" string would parse as UTC midnight).
function createPayload(ev) {
    var notes = ev.invite && ev.invite.length ? "with " + ev.invite.join(", ") : "";
    return {
        title: ev.title,
        start: parseTime(ev.start),
        end: parseTime(ev.end),
        allDay: !!ev.allDay,
        location: ev.location || "",
        notes: notes,
        calendar: ev.calendar
    };
}
