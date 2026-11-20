// node --test shell/end4-pC/modules/ii/claude/tests/appleLogic.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const src = readFileSync(new URL("../AppleLogic.js", import.meta.url), "utf8").replace(".pragma library", "");
const L = new Function(src + "\nreturn {" + [...src.matchAll(/^function (\w+)/gm)].map(m => m[1]).join(",") + "};")();

const now = new Date(2026, 10, 20, 12, 0); // fri 20 nov 2026, 12:00
const cals = L.calendarList({ targets: ["local", "iCloud: home", "iCloud: work"] });

test("calendarList splits account and name", () => {
  assert.deepEqual(cals.map(c => [c.id, c.account, c.name]), [["local", "", "local"], ["iCloud: home", "iCloud", "home"], ["iCloud: work", "iCloud", "work"]]);
});

test("intent routes", () => {
  assert.equal(L.intent("dinner with kuro sat 8"), "event");
  assert.equal(L.intent("when am I free this week"), "free");
  assert.equal(L.intent("play something for focus"), "queue");
  assert.equal(L.intent("why does this fail"), "other");
  assert.equal(L.intent("what is a monad"), "other");
  assert.equal(L.intent(""), "other");
});

test("replyText reads the cli json and bare text", () => {
  assert.deepEqual(L.replyText('{"type":"result","is_error":false,"result":"hi"}'), { ok: true, text: "hi" });
  assert.equal(L.replyText('{"is_error":true,"result":"Not logged in"}').ok, false);
  assert.equal(L.replyText("plain").text, "plain");
  assert.equal(L.replyText("  ").ok, false);
});

test("extractJson finds fenced and embedded objects", () => {
  assert.deepEqual(L.extractJson('Sure!\n```json\n{"a": {"b": "}"}}\n```'), { a: { b: "}" } });
  assert.deepEqual(L.extractJson('here {"a":1} done'), { a: 1 });
  assert.equal(L.extractJson("no json"), null);
  assert.equal(L.extractJson('{"a":'), null);
});

test("normalizeEvent: card lines, defaults, calendar pick", () => {
  const r = L.normalizeEvent({ title: "dinner with Kuro", start: "2026-11-21T20:00", end: "", calendar: "iCloud: home", invite: ["Kuro"] }, now, cals, "local");
  assert.ok(r.ok);
  assert.equal(r.event.end, "2026-11-21T21:00");
  assert.equal(r.event.calendar, "iCloud: home");
  assert.deepEqual(L.eventCardLines(r.event), ["dinner with Kuro", "sat 21 nov · 20:00–21:00", "iCloud: home · with Kuro"]);
  assert.equal(r.event.past, false);
  // unknown calendar falls back to the default, then the first writable
  assert.equal(L.normalizeEvent({ title: "x", start: "2026-11-21T20:00", calendar: "nope" }, now, cals, "iCloud: work").event.calendar, "iCloud: work");
  assert.equal(L.normalizeEvent({ title: "x", start: "2026-11-21T20:00", calendar: "nope" }, now, cals, "").event.calendar, "local");
  assert.equal(L.normalizeEvent({ title: "x", start: "2026-11-19T20:00" }, now, cals, "").event.past, true);
  assert.equal(L.normalizeEvent({ title: "", start: "2026-11-21T20:00" }, now, cals, "").ok, false);
  assert.equal(L.normalizeEvent({ title: "x", start: "soon" }, now, cals, "").ok, false);
  assert.equal(L.normalizeEvent([], now, cals, "").ok, false);
});

test("all-day events", () => {
  const r = L.normalizeEvent({ title: "trip", start: "2026-11-22", allDay: true }, now, cals, "");
  assert.equal(r.event.start, "2026-11-22");
  assert.equal(r.event.end, "2026-11-23");
  assert.equal(L.eventCardLines(r.event)[1], "sun 22 nov · all day");
});

test("createPayload gives Dates and notes", () => {
  const ev = L.normalizeEvent({ title: "dinner", start: "2026-11-21T20:00", invite: ["Kuro"], calendar: "iCloud: home" }, now, cals, "").event;
  const p = L.createPayload(ev);
  assert.ok(p.start instanceof Date);
  assert.equal(p.start.getHours(), 20);
  assert.equal(p.notes, "with Kuro");
  assert.equal(p.calendar, "iCloud: home");
});


test("applyEdit moves the end with the start", () => {
  const ev = L.normalizeEvent({ title: "dinner", start: "2026-11-21T20:00", end: "2026-11-21T22:00" }, now, cals, "").event;
  assert.equal(L.applyEdit(ev, "start", "2026-11-21T23:00").end, "2026-11-22T00:00");
  assert.equal(L.applyEdit(ev, "end", "2026-11-21T19:00"), null);
  assert.equal(L.applyEdit(ev, "start", "tomorrow"), null);
  assert.deepEqual(L.applyEdit(ev, "invite", "Kuro, Mio").invite, ["Kuro", "Mio"]);
});

const raw = [
  { title: "server night", start: "2026-11-21T20:00:00+01:00", end: "2026-11-21T23:00:00+01:00", allDay: false, calendar: "friends", account: "iCloud", notes: "bring the spare switch" },
  { title: "work thing", start: "2026-11-23T10:00:00", end: "2026-11-23T11:00:00", calendar: "work", account: "iCloud", notes: "secret agenda" },
  { title: "ancient", start: "2026-01-01T10:00:00", end: "2026-01-01T11:00:00", calendar: "home", account: "iCloud" },
  { title: "trip", start: "2026-11-22T00:00:00", end: "2026-11-23T00:00:00", allDay: true, calendar: "home", account: "iCloud" },
];

test("free time sends titles and times only", () => {
  const evs = L.eventList({ events: raw });
  const ctx = L.freeContext(evs, now);
  assert.equal(ctx.events.length, 3);
  assert.ok(ctx.summary.startsWith("reading iCloud: home, iCloud: friends, iCloud: work · 3 events"));
  const p = L.freePrompt("when am I free this week", ctx, now);
  assert.ok(p.includes("server night"));
  assert.ok(!p.includes("secret agenda") && !p.includes("spare switch"));
  assert.ok(!p.includes("ancient"));
});

test("note context, prompt, cleanup", () => {
  const ev = L.eventList({ events: [{ title: "server night", start: "2026-11-21T20:00:00", end: "2026-11-21T23:00:00", calendar: "friends", notes: "bring the spare switch", attachments: [{ name: "agenda.pdf", text: "1. NAS move" }, "photo.png"] }] })[0];
  assert.equal(L.noteSummary(ev), "reading invite + 1 attachment · 1 not readable");
  const p = L.notePrompt(ev);
  assert.ok(p.includes("spare switch") && p.includes("NAS move") && p.includes("--- agenda.pdf ---"));
  assert.equal(L.noteHasContent(L.eventList({ events: [{ title: "a", start: "2026-11-21T20:00:00" }] })[0]), false);
  assert.equal(L.cleanNote("**Bring** the switch."), "Bring the switch.");
  assert.equal(L.cleanNote("NOTHING"), "");
  assert.ok(L.cleanNote("x".repeat(400)).length <= 280);
});

test("queue only keeps library tracks", () => {
  const lib = L.libraryContext([{ id: "1", title: "Green", artist: "Hiroshi Yoshimura", seconds: 300 }, { id: "2", title: "Mbira", artist: "Midori Takada", seconds: 600 }]);
  const r = L.normalizeQueue({ name: "focus", tracks: [{ id: "2" }, { id: "9", title: "Made up", artist: "Nobody" }, { title: "green", artist: "hiroshi yoshimura" }, { id: "2" }] }, lib);
  assert.ok(r.ok);
  assert.deepEqual(r.queue.tracks.map(t => t.id), ["2", "1"]);
  assert.equal(r.queue.dropped, 1);
  assert.equal(r.queue.seconds, 900);
  assert.equal(L.duration(6720), "1h 52m");
  assert.equal(L.normalizeQueue({ tracks: [{ id: "x" }] }, lib).ok, false);
  const p = L.queuePrompt("focus", lib);
  assert.ok(p.includes("1 | Green | Hiroshi Yoshimura"));
});
