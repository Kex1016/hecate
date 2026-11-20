pragma Singleton
pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

// Calendar accounts: iCloud (CalDAV at caldav.icloud.com with an
// app-specific password), any other CalDAV server, and read-only .ics URLs.
//
// Accounts and the calendars picked from them are in
// ~/.config/hecate/calendars.toml. Passwords never are: each one is a keyring
// item (`secret-tool lookup application hecate service calendars account <id>`).
// This side signs in, lists an account's calendars and keeps the picks; the
// calendar service syncs the events and writes them, already filtered to the
// shown calendars, to ~/.local/state/quickshell/user/calendars/events.json:
//   [{ "title", "start", "end" (ISO 8601), "allDay", "account" (name),
//      "accountId", "calendar" (name), "href" (calendar href) }]
// and may record `synced` (ISO time) per account id in status.json there.
Singleton {
    id: root

    readonly property string path: Directories.calendarsPath
    readonly property string stateDir: FileUtils.trimFileProtocol(`${Directories.state}/user/calendars`)
    readonly property string icloudUrl: "https://caldav.icloud.com/"

    // [{ id, kind: "icloud" | "caldav" | "ics", name, user, url, home, refresh,
    //    calendars: [{ href, name, color, shown, writable, components }] }]
    property var accounts: []
    // { <id>: { state: "ok" | "checking" | "error", message, checked, synced } }
    property var status: ({})
    property var events: []
    property bool loaded: false

    // "local", then every shown calendar that takes events, as "<account>: <calendar>"
    readonly property var newEventTargets: {
        const out = ["local"];
        for (const a of root.accounts)
            for (const c of a.calendars ?? [])
                if (c.writable && c.shown && (c.components ?? ["VEVENT"]).includes("VEVENT"))
                    out.push(`${a.name}: ${c.name}`);
        return out;
    }

    // ── reading what the calendar service synced ──────────────────────────────────

    function shownEvent(e) {
        const a = root.accounts.find(x => x.id === e.accountId);
        if (!a) return true;
        const c = (a.calendars ?? []).find(x => x.href === e.href);
        return !c || c.shown;
    }
    function eventsIn(year, month) {
        return root.events.filter(e => {
            const d = new Date(e.start);
            return d.getFullYear() === year && d.getMonth() === month && root.shownEvent(e);
        });
    }
    function nextEvent(now) {
        const t = now.getTime();
        const ahead = root.events
            .filter(e => new Date(e.end ?? e.start).getTime() >= t && root.shownEvent(e))
            .sort((a, b) => new Date(a.start) - new Date(b.start));
        return ahead[0] ?? null;
    }

    // ── accounts ──────────────────────────────────────────────────────────────────

    function newId(kind) {
        return `${kind}-${Date.now().toString(36)}`;
    }

    function setStatus(id, patch) {
        const next = Object.assign({}, root.status);
        next[id] = Object.assign({}, next[id] ?? {}, patch);
        root.status = next;
        statusFile.setText(JSON.stringify(root.status, null, 2));
    }

    function dropStatus(id) {
        if (root.status[id] === undefined) return;
        const next = Object.assign({}, root.status);
        delete next[id];
        root.status = next;
        statusFile.setText(JSON.stringify(root.status, null, 2));
    }

    function account(id) {
        return root.accounts.find(a => a.id === id) ?? null;
    }

    function putAccount(acc) {
        const list = root.accounts.filter(a => a.id !== acc.id);
        const i = root.accounts.findIndex(a => a.id === acc.id);
        list.splice(i < 0 ? list.length : i, 0, acc);
        root.accounts = list;
        root.save();
    }

    function remove(id) {
        root.accounts = root.accounts.filter(a => a.id !== id);
        root.save();
        root.run(["secret-tool", "clear", "application", "hecate", "service", "calendars", "account", id], "", () => {});
        root.dropStatus(id);
    }

    function setShown(id, href, shown) {
        const a = root.account(id);
        if (!a) return;
        root.putAccount(Object.assign({}, a, {
            calendars: a.calendars.map(c => c.href === href ? Object.assign({}, c, { shown }) : c)
        }));
    }

    // Signs in, lists the calendars and keeps the account; the password goes to
    // the keyring only once the server took it. done(error or "").
    function addCalDav(kind, url, user, password, done, existingId, keepSecret) {
        const id = existingId ?? root.newId(kind);
        const name = kind === "icloud" ? "icloud" : root.hostName(url);
        root.setStatus(id, { state: "checking", message: "" });
        root.discover(url, user, password, result => {
            if (result.error) {
                if (root.account(id)) root.setStatus(id, { state: "error", message: result.error });
                else root.dropStatus(id);
                done(result.error);
                return;
            }
            const old = root.account(id);
            const store = keepSecret ? (i, l, p, cb) => cb(true) : root.storeSecret;
            store(id, `hecate calendar ${name} (${user})`, password, ok => {
                if (!ok) {
                    if (root.account(id)) root.setStatus(id, { state: "error", message: "The keyring would not take the password." });
                    else root.dropStatus(id);
                    done("The keyring would not take the password. Is a Secret Service (GNOME Keyring, KWallet) running, and secret-tool installed?");
                    return;
                }
                root.putAccount({
                    id, kind, name, user, url,
                    home: result.home,
                    refresh: 0,
                    calendars: root.mergeCalendars(old?.calendars ?? [], result.calendars)
                });
                root.setStatus(id, { state: "ok", message: "", checked: new Date().toISOString() });
                done("");
            });
        });
    }

    function addIcs(name, url, done) {
        const id = root.newId("ics");
        const u = url.trim().replace(/^webcal:\/\//i, "https://");
        root.setStatus(id, { state: "checking", message: "" });
        root.fetch(u, (code, body) => {
            if (code < 200 || code >= 300 || !/BEGIN:VCALENDAR/.test(body)) {
                const error = code === 0 ? "Could not reach that address." : `That address did not answer with a calendar (HTTP ${code}).`;
                root.dropStatus(id);
                done(error);
                return;
            }
            root.putAccount({
                id, kind: "ics", name: name.trim() || root.hostName(u), user: "", url: u, home: "",
                refresh: 1440,
                calendars: [{ href: u, name: name.trim() || root.hostName(u), color: "", shown: true, writable: false, components: ["VEVENT"] }]
            });
            root.setStatus(id, { state: "ok", message: "", checked: new Date().toISOString() });
            done("");
        });
    }

    // Keeps the shown/hidden picks of calendars that are still there.
    function mergeCalendars(old, found) {
        return found.map(c => {
            const before = old.find(o => o.href === c.href);
            // Reminders lists start hidden: CalDAV only reaches the old ones.
            const shown = before ? before.shown : (c.components ?? []).includes("VEVENT");
            return Object.assign({}, c, { shown });
        });
    }

    // Checks every account again: calendars added or removed upstream, a changed
    // password. The calendar service watches status.json's `syncRequested`.
    function requestSync() {
        const next = Object.assign({}, root.status, { syncRequested: new Date().toISOString() });
        root.status = next;
        statusFile.setText(JSON.stringify(root.status, null, 2));
        for (const a of root.accounts) root.recheck(a.id);
    }

    function recheck(id) {
        const a = root.account(id);
        if (!a) return;
        if (a.kind === "ics") {
            root.setStatus(id, { state: "checking" });
            root.fetch(a.url, (code, body) => {
                const ok = code >= 200 && code < 300 && /BEGIN:VCALENDAR/.test(body);
                root.setStatus(id, ok ? { state: "ok", message: "", checked: new Date().toISOString() }
                    : { state: "error", message: code === 0 ? "Could not reach that address." : `HTTP ${code}` });
            });
            return;
        }
        root.lookupSecret(id, password => {
            if (password === "") {
                root.setStatus(id, { state: "error", message: "No password in the keyring. Edit the account to sign in again." });
                return;
            }
            root.addCalDav(a.kind, a.url, a.user, password, () => {}, id, true);
        });
    }

    // ── CalDAV discovery (RFC 4791, 6764): principal, calendar home, calendars ─────

    readonly property string bodyPrincipal: '<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>'
    readonly property string bodyHome: '<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><c:calendar-home-set/></d:prop></d:propfind>'
    readonly property string bodyList: '<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="http://apple.com/ns/ical/"><d:prop><d:displayname/><d:resourcetype/><a:calendar-color/><c:supported-calendar-component-set/><d:current-user-privilege-set/></d:prop></d:propfind>'

    function discover(url, user, password, done) {
        const fail = (code, what) => done({ error: code === 401 || code === 403 ? "The server refused the user name or password."
            : code === 0 ? "Could not reach the server."
            : `The server answered ${code} while looking for ${what}.` });
        root.propfind(url, 0, root.bodyPrincipal, user, password, (code, body, effective) => {
            if (code !== 207) return fail(code, "your account");
            const principal = root.firstHref(body, "current-user-principal");
            const principalUrl = principal ? root.resolve(effective, principal) : effective;
            root.propfind(principalUrl, 0, root.bodyHome, user, password, (code2, body2, effective2) => {
                if (code2 !== 207) return fail(code2, "your calendars");
                const home = root.firstHref(body2, "calendar-home-set");
                // No home set: the address may already be the calendar home.
                const homeUrl = home ? root.resolve(effective2, home) : principalUrl;
                root.propfind(homeUrl, 1, root.bodyList, user, password, (code3, body3, effective3) => {
                    if (code3 !== 207) return fail(code3, "your calendars");
                    const calendars = root.parseCalendars(body3, effective3);
                    if (calendars.length === 0) return done({ error: "Signed in, but the account has no calendars." });
                    done({ home: homeUrl, calendars });
                });
            });
        });
    }

    // Namespace prefixes off, so <d:href>, <D:href> and <href xmlns=...> all read alike.
    function plain(xml) {
        return xml.replace(/<(\/?)[A-Za-z][\w.-]*:/g, "<$1").replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, (m, t) => root.xmlEscape(t));
    }
    function xmlEscape(s) {
        return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }
    function xmlUnescape(s) {
        return s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"").replace(/&apos;/g, "'").replace(/&#(\d+);/g, (m, n) => String.fromCharCode(parseInt(n))).replace(/&amp;/g, "&");
    }
    function inner(xml, tag) {
        const m = xml.match(new RegExp(`<${tag}(?:\\s[^>]*)?>([\\s\\S]*?)</${tag}>`));
        return m ? m[1] : null;
    }
    function firstHref(xml, prop) {
        const block = root.inner(root.plain(xml), prop);
        const href = block ? root.inner(block, "href") : null;
        return href ? root.xmlUnescape(href.trim()) : "";
    }

    function parseCalendars(xml, base) {
        const out = [];
        const doc = root.plain(xml);
        const responses = doc.match(/<response(?:\s[^>]*)?>[\s\S]*?<\/response>/g) ?? [];
        for (const r of responses) {
            // Properties that came back 200; a 404 propstat lists what the server lacks.
            const ok = (r.match(/<propstat(?:\s[^>]*)?>[\s\S]*?<\/propstat>/g) ?? []).filter(p => /<status>[^<]*\s200\s/.test(p)).join("");
            const type = root.inner(ok, "resourcetype") ?? "";
            if (!/<calendar[\s/>]/.test(type)) continue;
            const href = root.xmlUnescape((root.inner(r, "href") ?? "").trim());
            if (!href) continue;
            const comps = (root.inner(ok, "supported-calendar-component-set") ?? "").match(/name="([A-Z]+)"/g)?.map(s => s.slice(6, -1)) ?? ["VEVENT"];
            const privs = root.inner(ok, "current-user-privilege-set") ?? "";
            const writable = privs === "" || /<(write|write-content|all|bind)[\s/>]/.test(privs);
            const url = root.resolve(base, href);
            const name = root.xmlUnescape((root.inner(ok, "displayname") ?? "").trim()) || decodeURIComponent(href.replace(/\/$/, "").split("/").pop());
            out.push({
                href: url,
                name,
                color: (root.inner(ok, "calendar-color") ?? "").trim().slice(0, 7),
                shown: true,
                writable,
                components: comps.length > 0 ? comps : ["VEVENT"]
            });
        }
        return out;
    }

    function resolve(base, href) {
        if (/^https?:\/\//i.test(href)) return href;
        const m = base.match(/^(https?:\/\/[^/]+)(\/.*)?$/i);
        if (!m) return href;
        if (href.startsWith("/")) return m[1] + href;
        const dir = (m[2] ?? "/").replace(/[^/]*$/, "");
        return m[1] + dir + href;
    }

    function hostName(url) {
        const m = String(url).match(/^\w+:\/\/([^/:]+)/);
        return m ? m[1].replace(/^(www|caldav|cal|dav)\./, "") : "calendar";
    }

    // Writes "me@icloud.com" as "········@icloud.com" for the page.
    function masked(user) {
        const at = String(user).indexOf("@");
        return at > 0 ? "·".repeat(Math.min(8, at)) + user.slice(at) : user;
    }

    // ── curl and secret-tool, with secrets only ever on stdin ────────────────────

    // curl reads its whole config from stdin (-K -), so neither the password nor
    // the request body is on a command line. The last line is "<code> <url>".
    function curlConfig(lines) {
        return lines.map(([k, v]) => v === undefined ? k : `${k} = "${String(v).replace(/\\/g, "\\\\").replace(/"/g, "\\\"").replace(/\n/g, "\\n").replace(/\r/g, "\\r").replace(/\t/g, "\\t")}"`).join("\n") + "\n";
    }
    // The password goes out with Basic auth, so only over https (or to this machine).
    function local(url) {
        return /^\w+:\/\/(localhost|127\.0\.0\.1|\[::1\])([:/]|$)/i.test(String(url));
    }
    function propfind(url, depth, body, user, password, done) {
        const proto = root.local(url) ? [] : [["proto", "=https"], ["proto-redir", "=https"]];
        const config = root.curlConfig(proto.concat([
            ["url", url],
            ["request", "PROPFIND"],
            ["header", `Depth: ${depth}`],
            ["header", "Content-Type: application/xml; charset=utf-8"],
            ["data-binary", body],
            ["user", `${user}:${password}`],
            ["location"],
            ["silent"],
            ["max-time", "20"],
            ["write-out", "\n%{http_code} %{url_effective}"],
        ]));
        root.run(["curl", "-K", "-"], config, (exit, out) => root.splitCurl(out, url, done));
    }
    function fetch(url, done) {
        const config = root.curlConfig([
            ["url", url],
            ["location"],
            ["silent"],
            ["max-time", "20"],
            ["write-out", "\n%{http_code} %{url_effective}"],
        ]);
        root.run(["curl", "-K", "-"], config, (exit, out) => root.splitCurl(out, url, (code, body) => done(code, body)));
    }
    function splitCurl(out, url, done) {
        const nl = out.lastIndexOf("\n");
        const tail = (nl >= 0 ? out.slice(nl + 1) : out).trim().split(" ");
        const code = parseInt(tail[0]) || 0;
        done(code, nl >= 0 ? out.slice(0, nl) : "", tail[1] || url);
    }

    function storeSecret(id, label, password, done) {
        root.run(["secret-tool", "store", `--label=${label}`, "application", "hecate", "service", "calendars", "account", id], password, exit => done(exit === 0));
    }
    function lookupSecret(id, done) {
        root.run(["secret-tool", "lookup", "application", "hecate", "service", "calendars", "account", id], "", (exit, out) => done(exit === 0 ? out : ""));
    }

    // Through sh, so a missing binary still ends the job (exit 127) instead of
    // never starting and never calling back.
    function run(command, input, done) {
        const wrapped = ["sh", "-c", 'command -v "$1" >/dev/null 2>&1 || exit 127; exec "$@"', "sh"].concat(command);
        const job = jobComponent.createObject(root, { command: wrapped, input, done });
        job.running = true;
    }

    Component {
        id: jobComponent
        Process {
            id: job
            property string input: ""
            property var done: null
            property int exit: -1
            property string out: ""
            property int pending: 2
            stdinEnabled: true
            stdout: StdioCollector {
                onStreamFinished: {
                    job.out = text;
                    job.finish();
                }
            }
            onRunningChanged: {
                if (!running) return;
                if (job.input !== "") job.write(job.input);
                job.stdinEnabled = false;
            }
            onExited: code => {
                job.exit = code;
                job.finish();
            }
            function finish() {
                job.pending -= 1;
                if (job.pending > 0) return;
                job.done?.(job.exit, job.out);
                job.destroy();
            }
        }
    }

    // ── calendars.toml ────────────────────────────────────────────────────────────

    // The subset this file uses: [[account]], [[account.calendar]], and
    // key = "string" | true | false | number | ["strings"].
    function parse(text) {
        const accounts = [];
        let target = null;
        for (const raw of text.split("\n")) {
            const line = raw.trim();
            if (line === "" || line.startsWith("#")) continue;
            if (line === "[[account]]") {
                target = { calendars: [] };
                accounts.push(target);
            } else if (line === "[[account.calendar]]") {
                const owner = accounts[accounts.length - 1];
                if (!owner) continue;
                target = {};
                owner.calendars.push(target);
            } else if (target) {
                const m = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
                if (!m) continue;
                try {
                    target[m[1]] = JSON.parse(m[2]);
                } catch (e) {
                    console.warn(`[CalendarAccounts] ${root.path}: cannot read "${line}"`);
                }
            }
        }
        return accounts.filter(a => a.id && a.kind);
    }

    function toToml(accounts) {
        const v = x => JSON.stringify(x);
        let out = "# Calendar accounts, written by the dashboard's calendars page.\n"
            + "# Passwords are not here but in the keyring:\n"
            + "#   secret-tool lookup application hecate service calendars account <id>\n";
        for (const a of accounts) {
            out += `\n[[account]]\nid = ${v(a.id)}\nkind = ${v(a.kind)}\nname = ${v(a.name)}\n`;
            if (a.user) out += `user = ${v(a.user)}\n`;
            out += `url = ${v(a.url)}\n`;
            if (a.home) out += `home = ${v(a.home)}\n`;
            if (a.refresh) out += `refresh = ${a.refresh}\n`;
            for (const c of a.calendars ?? [])
                out += `\n[[account.calendar]]\nhref = ${v(c.href)}\nname = ${v(c.name)}\ncolor = ${v(c.color ?? "")}\nshown = ${c.shown ? "true" : "false"}\nwritable = ${c.writable ? "true" : "false"}\ncomponents = ${v(c.components ?? ["VEVENT"])}\n`;
        }
        return out;
    }

    function save() {
        tomlFile.setText(root.toToml(root.accounts));
    }

    Component.onCompleted: {
        Quickshell.execDetached(["mkdir", "-p", Directories.hecateConfig, root.stateDir]);
    }

    FileView {
        id: tomlFile
        path: root.path
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            root.accounts = root.parse(tomlFile.text());
            root.loaded = true;
        }
        onLoadFailed: root.loaded = true
    }
    FileView {
        id: statusFile
        path: `${root.stateDir}/status.json`
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                root.status = JSON.parse(statusFile.text()) ?? {};
            } catch (e) {
                root.status = {};
            }
        }
    }
    FileView {
        id: eventsFile
        path: `${root.stateDir}/events.json`
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: {
            try {
                const list = JSON.parse(eventsFile.text());
                root.events = Array.isArray(list) ? list : [];
            } catch (e) {
                root.events = [];
            }
        }
    }
}
