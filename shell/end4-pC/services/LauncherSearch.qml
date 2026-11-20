pragma Singleton

import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io
import "../modules/ii/overview/LauncherMatch.js" as Match

// What the launcher (Mod+A) shows for a query. The query keeps its prefix
// (`v foo`, `tr hello`); `mode` and `text` are the parsed halves. Results
// come in groups of plain rows:
//   { key, group, text, alias, match, meta, ink, hint, execute(), ... }
// Prefixes are fixed: > run, = calculate, v clipboard, : emoji,
// tr translate, / files under ~, ? ask Claude (a hook, see claudeHook).
Singleton {
    id: root

    property string query: ""
    readonly property var parsed: Match.parse(root.query)
    readonly property string mode: parsed.mode
    readonly property string text: parsed.text
    readonly property var prefixes: Match.PREFIXES

    // Claude (`?`). Wave 2's ask panel sets this to a function(text) that
    // takes the question; until then the `?` row is a placeholder.
    property var claudeHook: null
    readonly property bool claudeReady: typeof root.claudeHook === "function"

    // Contacts (iCloud and other CardDAV address books): shell.qml sets this to
    // function(query, count) → [contact], best match first (services/apple/Contacts).
    property var contactsSearch: null

    // Puts the launcher in a mode, keeping what was typed after any prefix.
    function setMode(mode) {
        root.query = Match.prefixFor(mode) + root.text;
    }
    // Old callers pass a prefix string.
    function ensurePrefix(prefix) {
        root.query = prefix + root.text;
    }

    // ---- aliases: `launcher.aliases` in the shell config ----
    //   "launcher": { "aliases": { "files": "yazi", "music": ["spotify", "cider"] } }
    // A target is a desktop id (with or without .desktop), its last
    // reverse-DNS part, or an app name; anything else runs as a command.
    // The panels settings page edits the same key.
    readonly property var aliases: Match.normalizeAliases(Config.options?.launcher?.aliases ?? {})

    // ---- helpers ----
    function row(group, fields) {
        return Object.assign({
            key: group + ":" + (fields.text ?? ""),
            group: group,
            text: "",
            alias: "",
            match: "",
            meta: "",
            ink: "text",
            hint: "open",
            execute: () => {}
        }, fields);
    }

    function runDetached(command) {
        Quickshell.execDetached(["bash", "-c", command]);
    }

    function runInTerminal(command) {
        const term = Config.options?.apps?.terminal ?? "kitty -1";
        Quickshell.execDetached(["bash", "-c", `${term} -e bash -c '${StringUtils.shellSingleQuoteEscape(command)}; exec bash'`]);
    }

    // Types text (or presses Ctrl+V) into the window that had focus, once
    // the launcher has closed. Falls back to only copying.
    function typeIntoFocused(textToType) {
        const t = StringUtils.shellSingleQuoteEscape(textToType ?? "");
        root.runDetached(textToType === undefined
            ? "sleep 0.2; if command -v wtype >/dev/null; then wtype -M ctrl -k v -m ctrl; elif command -v ydotool >/dev/null; then ydotool key 29:1 47:1 47:0 29:0; fi"
            : `sleep 0.2; command -v wtype >/dev/null && wtype -- '${t}'`);
    }

    readonly property string webEngineName: {
        const m = (Config.options?.search?.engineBaseUrl ?? "").match(/^https?:\/\/(?:www\.)?([^./]+)/);
        const host = m ? m[1] : "web";
        return host === "duckduckgo" ? "ddg" : host;
    }

    function openApp(entry) {
        if (!entry.runInTerminal)
            entry.execute();
        else
            root.runInTerminal(entry.command.join(" "));
    }

    function appMeta(entry) {
        if (entry.genericName)
            return entry.genericName.toLowerCase();
        const cats = { AudioVideo: "audio", Audio: "audio", Video: "video", Development: "development", Education: "education", Game: "game", Graphics: "graphics", Network: "internet", Office: "office", Science: "science", Settings: "settings", System: "system", Utility: "utility" };
        for (const c of (entry.categories ?? []))
            if (cats[c])
                return cats[c];
        return "";
    }

    // ---- async sources: qalc, trans, files ----
    property string mathFor: ""
    property string mathResult: ""
    property string translateFor: ""
    property string translateResult: ""
    property bool translateBusy: false
    property string filesFor: ""
    property var fileHits: []

    Timer {
        id: asyncTimer
        interval: Config.options?.search?.nonAppResultDelay ?? 30
        onTriggered: root.startAsync()
    }
    onQueryChanged: asyncTimer.restart()

    function startAsync() {
        const t = root.text.trim();
        const wantsMath = root.mode === "calc" || (root.mode === "search" && Match.looksLikeMath(t));
        if (wantsMath && t.length > 0 && t !== root.mathFor) {
            mathProc.running = false;
            mathProc.expr = t;
            mathProc.running = true;
        }
        if (root.mode === "translate" && t.length > 0 && t !== root.translateFor) {
            transDelay.restart();
        }
        const wantsFiles = (root.mode === "files" || root.mode === "search") && t.length > (root.mode === "files" ? 0 : 1);
        if (wantsFiles && t !== root.filesFor) {
            filesDelay.restart();
        } else if (!wantsFiles) {
            root.fileHits = [];
            root.filesFor = "";
        }
    }

    Process {
        id: mathProc
        property string expr: ""
        command: ["qalc", "-t", expr]
        stdout: StdioCollector {
            onStreamFinished: {
                root.mathFor = mathProc.expr;
                root.mathResult = text.trim();
            }
        }
    }

    // trans is slow and online: wait for a pause in typing
    Timer {
        id: transDelay
        interval: 400
        onTriggered: {
            const tr = Config.options?.language?.translator;
            transProc.running = false;
            transProc.source = root.text.trim();
            transProc.command = ["trans", "-brief", "-no-ansi",
                "-e", tr?.engine && tr.engine !== "auto" ? tr.engine : "google",
                "-s", tr?.sourceLanguage && tr.sourceLanguage !== "auto" ? tr.sourceLanguage : "auto",
                "-t", tr?.targetLanguage && tr.targetLanguage !== "auto" ? tr.targetLanguage : "en",
                transProc.source];
            root.translateBusy = true;
            transProc.running = true;
        }
    }

    Process {
        id: transProc
        property string source: ""
        stdout: StdioCollector {
            onStreamFinished: {
                root.translateFor = transProc.source;
                root.translateResult = text.trim();
                root.translateBusy = false;
            }
        }
    }

    // fd when it is there, else find; prints "mtime<TAB>path" relative to ~
    Timer {
        id: filesDelay
        interval: 150
        onTriggered: {
            filesProc.running = false;
            filesProc.needle = root.text.trim();
            filesProc.command = ["bash", "-c", root.filesScript, "launcher-files", filesProc.needle, root.mode === "files" ? "40" : "12"];
            filesProc.running = true;
        }
    }

    readonly property string filesScript: "q=$1; n=$2; cd \"$HOME\" || exit 0\n"
        + "{ if command -v fd >/dev/null; then timeout 3 fd --type f --fixed-strings --ignore-case --max-results \"$n\" -- \"$q\" . ;\n"
        + "  else g=${q//[\\*\\?\\[\\]]/}; timeout 3 find . -maxdepth 6 \\( -path '*/.*' -o -name node_modules -o -name target \\) -prune -o -type f -iname \"*$g*\" -print | head -n \"$n\"; fi; } 2>/dev/null |\n"
        + "while IFS= read -r f; do f=${f#./}; printf '%s\\t%s\\n' \"$(stat -c %Y -- \"$f\" 2>/dev/null || echo 0)\" \"$f\"; done"

    Process {
        id: filesProc
        property string needle: ""
        stdout: StdioCollector {
            onStreamFinished: {
                const now = Date.now() / 1000;
                const needle = filesProc.needle.toLowerCase();
                const hits = text.split("\n").filter(l => l.includes("\t")).map(l => {
                    const tab = l.indexOf("\t");
                    const path = l.slice(tab + 1);
                    const base = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
                    return { path: path, mtime: parseInt(l.slice(0, tab)) || 0, starts: base.startsWith(needle) };
                });
                hits.sort((a, b) => (b.starts - a.starts) || (b.mtime - a.mtime));
                root.filesFor = filesProc.needle;
                root.fileHits = hits.map(h => ({ path: h.path, age: Match.age(now - h.mtime) }));
            }
        }
    }

    // ---- emoji ----
    property list<string> recentEmoji: []
    readonly property var symbols: [
        ["★", "black star"], ["☆", "white star"], ["☼", "sun"], ["✦", "four pointed star"], ["✧", "white four pointed star"],
        ["⁂", "asterism"], ["»", "right guillemet"], ["«", "left guillemet"], ["♪", "eighth note"], ["♫", "beamed notes"],
        ["→", "right arrow"], ["←", "left arrow"], ["↑", "up arrow"], ["↓", "down arrow"], ["•", "bullet"],
        ["·", "middle dot"], ["…", "ellipsis"], ["—", "em dash"], ["°", "degree"], ["×", "multiplication"],
        ["÷", "division"], ["±", "plus minus"], ["≈", "almost equal"], ["≠", "not equal"], ["∞", "infinity"],
        ["♥", "heart suit"], ["♦", "diamond suit"], ["♣", "club suit"], ["♠", "spade suit"], ["☺", "smiling face"]
    ]

    function emojiRow(glyph, name, group) {
        return root.row(group, {
            key: group + ":" + glyph,
            text: glyph,
            meta: name,
            hint: "type it",
            emoji: true,
            execute: () => {
                Quickshell.clipboardText = glyph;
                root.rememberEmoji(glyph, name);
                root.typeIntoFocused(glyph);
            },
            copy: () => {
                Quickshell.clipboardText = glyph;
                root.rememberEmoji(glyph, name);
            }
        });
    }

    function rememberEmoji(glyph, name) {
        const entry = glyph + " " + name;
        root.recentEmoji = [entry].concat(root.recentEmoji.filter(e => e !== entry)).slice(0, 20);
    }

    // ---- the groups ----
    readonly property var groups: {
        const t = root.text.trim();
        switch (root.mode) {
        case "run": {
            if (!t) return [];
            return [{ name: "run", rows: [
                root.row("run", { text: t, ink: "gold", meta: "sh", hint: "run", execute: () => {
                    t.startsWith("sudo ") ? root.runInTerminal(t) : root.runDetached(t);
                } }),
                root.row("run", { key: "run:term", text: t, meta: "in a terminal", hint: "run", execute: () => root.runInTerminal(t) })
            ] }];
        }
        case "calc": {
            if (!t) return [];
            const ready = root.mathFor === t && root.mathResult.length > 0;
            return [{ name: "calculate", rows: [root.row("calculate", {
                text: ready ? `${t} → ${root.mathResult}` : t,
                meta: "qalc", hint: "copy",
                execute: () => { Quickshell.clipboardText = root.mathResult; }
            })] }];
        }
        case "clipboard": {
            const entries = t.length > 0 ? Cliphist.fuzzyQuery(t) : Cliphist.entries;
            const pinned = [], rest = [];
            for (const e of entries.slice(0, 60))
                (Cliphist.isPinned(e) ? pinned : rest).push(e);
            const rows = pinned.concat(rest).map((entry, i) => {
                const isImage = Cliphist.entryIsImage(entry);
                const info = isImage ? (entry.match(/(\w+)\s+(\d+x\d+)/) ?? []) : [];
                const isPinned = Cliphist.isPinned(entry);
                return root.row("clipboard", {
                    key: "clip:" + entry,
                    text: isImage ? "" : StringUtils.cleanCliphistEntry(entry),
                    match: t,
                    meta: isPinned ? "pinned" : "",
                    hint: "paste",
                    clip: entry,
                    pinned: isPinned,
                    image: isImage,
                    imageInfo: isImage ? `${(info[2] ?? "").replace("x", "×")} ${info[1] ?? ""}`.trim() : "",
                    execute: () => {
                        Cliphist.copy(entry);
                        root.typeIntoFocused();
                    }
                });
            });
            return [{ name: "clipboard", rows: rows }];
        }
        case "emoji": {
            const out = [];
            if (!t && root.recentEmoji.length > 0)
                out.push({ name: "recent", grid: true, rows: root.recentEmoji.map(e => root.emojiRow(e.split(" ")[0], e.slice(e.indexOf(" ") + 1), "recent")) });
            const found = (t ? Emojis.fuzzyQuery(t) : Emojis.list).slice(0, 80);
            if (found.length > 0)
                out.push({ name: t ? "emoji" : "all", grid: true, rows: found.map(e => {
                    const glyph = e.match(/^\s*(\S+)/)?.[1] ?? "";
                    return root.emojiRow(glyph, e.replace(/^\s*\S+\s+/, ""), "emoji");
                }) });
            const sym = root.symbols.filter(s => !t || s[1].includes(t.toLowerCase()));
            if (sym.length > 0)
                out.push({ name: "symbols", grid: true, rows: sym.map(s => root.emojiRow(s[0], s[1], "symbols")) });
            return out;
        }
        case "translate": {
            if (!t) return [];
            const ready = root.translateFor === t && root.translateResult.length > 0;
            return [{ name: "translate", rows: [root.row("translate", {
                text: ready ? root.translateResult : (root.translateBusy ? "translating…" : t),
                ink: ready ? "text" : "inkMuted",
                meta: "trans", hint: "copy",
                execute: () => { if (ready) Quickshell.clipboardText = root.translateResult; }
            })] }];
        }
        case "files":
            return root.fileGroup(t, 40);
        case "claude":
            return [{ name: "claude", rows: [root.row("claude", {
                key: "claude:ask",
                text: t ? `ask claude: ${t}` : "ask claude",
                ink: root.claudeReady ? "text" : "inkMuted",
                meta: root.claudeReady ? "Mod+/" : "not set up yet",
                hint: "ask",
                placeholder: !root.claudeReady,
                execute: () => { if (root.claudeReady) root.claudeHook(t); }
            })] }];
        default:
            return root.searchGroups(t);
        }
    }

    function fileGroup(t, limit) {
        if (!t || root.filesFor !== t || root.fileHits.length === 0)
            return [];
        return [{ name: "files", rows: root.fileHits.slice(0, limit).map(h => root.row("files", {
            key: "file:" + h.path,
            text: "~/" + h.path,
            match: t,
            ink: "iris",
            meta: h.age,
            execute: () => Qt.openUrlExternally("file://" + Directories.home.replace(/^file:\/\//, "") + "/" + h.path)
        })) }];
    }

    function searchGroups(t) {
        if (!t)
            return [];
        const out = [];

        const apps = AppSearch.query(t, root.aliases).slice(0, 8).map(r => root.row("apps", {
            key: "app:" + r.entry.id,
            text: Match.label(r.entry.name, r.alias),
            alias: r.alias,
            // the alias is what matched, so that is what lights up
            match: r.alias ? "· " + r.alias : t,
            meta: root.appMeta(r.entry),
            entry: r.entry,
            execute: () => root.openApp(r.entry)
        }));
        const commands = AppSearch.commandAliases(t, root.aliases).map(c => root.row("apps", {
            key: "alias:" + c.command,
            text: Match.label(c.command, c.alias),
            alias: c.alias,
            match: "· " + c.alias,
            meta: "command",
            hint: "run",
            execute: () => root.runDetached(c.command)
        }));
        if (commands.length + apps.length > 0)
            out.push({ name: "apps", rows: commands.concat(apps) });

        out.push(...root.fileGroup(t, 3));

        // enter copies the number, or the e-mail when there is no number
        const people = typeof root.contactsSearch === "function" ? root.contactsSearch(t, 3) : [];
        const contacts = people.map(c => {
            const phone = c.phones?.[0]?.value ?? "";
            const email = c.emails?.[0]?.value ?? "";
            return root.row("contacts", {
                key: "contact:" + (c.uid ?? c.name),
                text: c.name ?? "",
                match: t,
                meta: String(c.account ?? "") !== "" ? `${String(c.account).toLowerCase()} contacts` : "contacts",
                hint: phone ? "copy number" : email ? "copy email" : "",
                execute: () => { if (phone || email) Quickshell.clipboardText = phone || email; }
            });
        });
        if (contacts.length > 0)
            out.push({ name: "contacts", rows: contacts });

        let settings = [];
        try {
            settings = (SettingsSearchIndex.search?.(t, 3) ?? []).map(e => root.row("settings", {
                key: "set:" + e.pageId + ":" + e.label,
                text: e.label,
                match: t,
                meta: e.kind === "page" ? "settings" : [e.pageName, e.section].filter(p => p).join(" › ").toLowerCase(),
                execute: () => GlobalStates.openSettingsAt(e.pageId, e.kind === "page" ? "" : e.label, e.section, e.subsection)
            }));
        } catch (e) {
            settings = [];
        }
        if (settings.length > 0)
            out.push({ name: "settings", rows: settings });

        const actions = [];
        if (Match.looksLikeMath(t) && root.mathFor === t && root.mathResult.length > 0)
            actions.push(root.row("actions", {
                key: "act:math", text: `${t} → ${root.mathResult}`, meta: "qalc", hint: "copy",
                execute: () => { Quickshell.clipboardText = root.mathResult; }
            }));
        actions.push(root.row("actions", {
            key: "act:web",
            text: `search the web for "${t}"`,
            verb: "search the web",
            meta: root.webEngineName,
            hint: "search",
            execute: () => {
                let url = (Config.options?.search?.engineBaseUrl ?? "https://duckduckgo.com/?q=") + encodeURIComponent(t);
                for (const site of (Config.options?.search?.excludedSites ?? []))
                    url += encodeURIComponent(` -site:${site}`);
                Qt.openUrlExternally(url);
            }
        }));
        out.push({ name: "actions", rows: actions });
        return out;
    }

    // Flat list in display order, each row with its group index.
    readonly property var rows: {
        const flat = [];
        root.groups.forEach((g, gi) => g.rows.forEach(r => flat.push(Object.assign({ groupIndex: gi }, r))));
        return flat;
    }
}
