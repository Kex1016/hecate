pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

// Contacts from the CardDAV address books of the calendar accounts: iCloud's
// (contacts.icloud.com, same Apple ID and app-specific password) and, for other
// CalDAV servers, the server's own CardDAV side when it has one. Read-only.
//
// Calendars' sync (scripts/apple/dav.py) fetches them, so they refresh on the
// same schedule, and writes ~/.local/state/quickshell/user/contacts/contacts.json:
//   [{ uid, name, first, last, nickname, org, title,
//      emails: [{ type, value }], phones: [{ type, value }],
//      birthday ("YYYY-MM-DD", or "--MM-DD" without a year), note,
//      account, accountId, addressbook }]
//
//   Contacts.lookup("anna")              → the best match, or null
//   Contacts.search("kov", 5)            → matches, best first
//   Contacts.byEmail("a@b.hu"), Contacts.byPhone("+36 30 123 4567")
Singleton {
    id: root

    property var contacts: []
    // contacts.json has been read (it may be empty).
    property bool ready: false
    readonly property int count: root.contacts.length
    readonly property bool syncing: Calendars.syncing

    readonly property string path: Calendars.contactsPath

    // The contact a name most likely means ("anna", "Anna K", "kovács"), or null.
    function lookup(name) {
        return root.search(name, 1)[0] ?? null;
    }

    // Contacts matching a name, nickname, organisation, e-mail or phone, best first.
    function search(query, count) {
        const q = root.fold(query);
        if (q === "") return [];
        const words = q.split(/\s+/);
        const scored = [];
        for (const c of root.contacts) {
            const s = root.score(c, q, words);
            if (s > 0) scored.push({ c, s });
        }
        scored.sort((a, b) => b.s - a.s || String(a.c.name).localeCompare(String(b.c.name)));
        return scored.slice(0, count ?? 10).map(x => x.c);
    }

    function byEmail(address) {
        const a = String(address ?? "").trim().toLowerCase();
        if (a === "") return null;
        return root.contacts.find(c => (c.emails ?? []).some(e => String(e.value).toLowerCase() === a)) ?? null;
    }

    function byPhone(number) {
        const n = root.digits(number);
        if (n.length < 6) return null;
        // the last 9 digits: "+36 30 123 4567" and "06301234567" are the same phone
        const tail = n.slice(-9);
        return root.contacts.find(c => (c.phones ?? []).some(p => root.digits(p.value).slice(-9) === tail)) ?? null;
    }

    // Contacts whose birthday falls in a month (0-11).
    function birthdaysIn(month) {
        return root.contacts.filter(c => {
            const m = String(c.birthday ?? "").match(/(\d\d)-(\d\d)$/);
            return m && parseInt(m[1]) - 1 === month;
        });
    }

    // Fetches again (with the calendars).
    function sync() {
        Calendars.sync();
    }

    // ── matching ──────────────────────────────────────────────────────────────────

    // lower case, accents off: "Kovács Ánna" → "kovacs anna"
    function fold(s) {
        return String(s ?? "").normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/\s+/g, " ").trim();
    }
    function digits(s) {
        return String(s ?? "").replace(/[^0-9]/g, "");
    }

    function score(c, q, words) {
        const name = root.fold(c.name);
        const first = root.fold(c.first);
        const last = root.fold(c.last);
        const nick = root.fold(c.nickname);
        const org = root.fold(c.org);
        if (q === name || (nick !== "" && q === nick)) return 100;
        // "anna kovacs" against "Kovács Anna"
        const parts = name.split(" ").concat(nick.split(" ")).filter(p => p !== "");
        if (words.length > 1 && words.every(w => parts.includes(w))) return 95;
        if (q === first) return 90;
        if (q === last) return 85;
        if (words.every(w => parts.some(p => p.startsWith(w)))) return 70 - Math.min(20, Math.abs(name.length - q.length));
        if (name.includes(q) || (nick !== "" && nick.includes(q))) return 50;
        if (org !== "" && (org === q || org.startsWith(q))) return 40;
        if (q.includes("@") && (c.emails ?? []).some(e => String(e.value).toLowerCase().startsWith(q))) return 60;
        const d = root.digits(q);
        if (d.length >= 6 && d.length === q.replace(/[\s+()-]/g, "").length
                && (c.phones ?? []).some(p => root.digits(p.value).endsWith(d.slice(-9)))) return 60;
        return 0;
    }

    function load() {
        Calendars.load();
    }

    FileView {
        id: file
        path: root.path
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                const list = JSON.parse(file.text());
                root.contacts = Array.isArray(list) ? list : [];
            } catch (e) {
                root.contacts = [];
            }
            root.ready = true;
        }
        onLoadFailed: root.ready = true
    }
}
