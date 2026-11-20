.pragma library

// Pure matching and ranking for the launcher, kept free of QML types so it
// can be unit-checked with node (tests/launcherMatch.test.mjs).

// The fixed prefixes. Letter prefixes need a space after them, so typing
// "vlc" or "transmission" stays a normal search.
var PREFIXES = [
    { token: ">", mode: "run", text: "run a command" },
    { token: "=", mode: "calc", text: "calculate", note: "= 2^10 → 1024" },
    { token: "v ", mode: "clipboard", text: "clipboard", note: "(Mod+V)" },
    { token: ":", mode: "emoji", text: "emoji", note: "(Mod+.)" },
    { token: "tr ", mode: "translate", text: "translate", note: "(was left sidebar)" },
    { token: "/", mode: "files", text: "files under ~" },
    { token: "?", mode: "claude", text: "ask claude" },
];

// "v foo" -> { mode: "clipboard", prefix: "v ", text: "foo" }
function parse(query) {
    var q = query || "";
    for (var i = 0; i < PREFIXES.length; i++) {
        var p = PREFIXES[i];
        if (q.indexOf(p.token) === 0)
            return { mode: p.mode, prefix: p.token, text: q.slice(p.token.length) };
    }
    return { mode: "search", prefix: "", text: q };
}

function prefixFor(mode) {
    for (var i = 0; i < PREFIXES.length; i++)
        if (PREFIXES[i].mode === mode)
            return PREFIXES[i].token;
    return "";
}

function norm(s) {
    return (s || "").toString().toLowerCase().trim();
}

// How well `text` matches `q` (both lower case): 100 exact, 80 prefix,
// 60 a later word starts with it, 40 substring, under 30 for a scattered
// subsequence (more for consecutive runs and word starts), 0 no match.
function score(text, q) {
    if (!q || !text)
        return 0;
    if (text === q)
        return 100;
    if (text.indexOf(q) === 0)
        return 80;
    var at = text.indexOf(q);
    if (at > 0) {
        var before = text.charAt(at - 1);
        return /[\s\-_.\/]/.test(before) ? 60 : 40;
    }
    // subsequence
    var ti = 0, run = 0, best = 0, bonus = 0;
    for (var qi = 0; qi < q.length; qi++) {
        var c = q.charAt(qi);
        var found = text.indexOf(c, ti);
        if (found < 0)
            return 0;
        if (found === ti && qi > 0)
            run++;
        else
            run = 0;
        best = Math.max(best, run);
        if (found === 0 || /[\s\-_.\/]/.test(text.charAt(found - 1)))
            bonus++;
        ti = found + 1;
    }
    var quality = (best + bonus) / q.length; // 0..2
    return Math.max(1, Math.min(29, Math.round(5 + quality * 12)));
}

// Accepts { alias: "target" | ["target", ...] } or [{ alias, app }].
// Returns { alias(lower case): [target, ...] }.
function normalizeAliases(config) {
    var out = {};
    if (!config)
        return out;
    function add(alias, target) {
        var a = norm(alias);
        if (!a || target === undefined || target === null)
            return;
        var list = Array.isArray(target) ? target : [target];
        out[a] = (out[a] || []).concat(list.map(function (t) { return String(t); }));
    }
    if (Array.isArray(config)) {
        config.forEach(function (e) {
            if (e)
                add(e.alias, e.app !== undefined ? e.app : e.target);
        });
    } else if (typeof config === "object") {
        Object.keys(config).forEach(function (k) { add(k, config[k]); });
    }
    return out;
}

// Does an alias target name this app? By desktop id (with or without
// .desktop, or its last reverse-DNS part) or by name.
function targets(app, target) {
    var t = norm(target).replace(/\.desktop$/, "");
    if (!t)
        return false;
    var id = norm(app.id).replace(/\.desktop$/, "");
    if (id === t || norm(app.name) === t)
        return true;
    var parts = id.split(".");
    return parts.length > 1 && parts[parts.length - 1] === t;
}

// Ranks apps for a query. `apps`: [{ id, name, genericName, keywords: [] }].
// Returns [{ app, score, alias, field }] best first; `alias` is the user
// alias or desktop-entry keyword that matched (empty when the name did).
//   exact user alias 1000, exact name 900, exact keyword 850,
//   name prefix 700, alias prefix 650, name word 600, keyword prefix 550,
//   generic name 400-500, name substring 350, keyword/alias word 300,
//   keyword/alias substring 200, scattered name letters 100-129.
function rank(apps, query, aliases) {
    var q = norm(query);
    if (!q)
        return [];
    var table = aliases || {};
    var userAliases = Object.keys(table);
    var out = [];
    for (var i = 0; i < apps.length; i++) {
        var app = apps[i];
        var best = { score: 0, alias: "", field: "" };
        var consider = function (s, alias, field) {
            if (s > best.score)
                best = { score: s, alias: alias, field: field };
        };

        var n = score(norm(app.name), q);
        if (n === 100) consider(900, "", "name");
        else if (n === 80) consider(700, "", "name");
        else if (n === 60) consider(600, "", "name");
        else if (n === 40) consider(350, "", "name");
        else if (n > 0) consider(100 + n, "", "name");

        for (var u = 0; u < userAliases.length; u++) {
            var a = userAliases[u];
            var hits = table[a].some(function (t) { return targets(app, t); });
            if (!hits)
                continue;
            var s = score(a, q);
            if (s === 100) consider(1000, a, "alias");
            else if (s === 80) consider(650, a, "alias");
            else if (s === 60) consider(300, a, "alias");
            else if (s === 40) consider(200, a, "alias");
        }

        var kws = app.keywords || [];
        for (var k = 0; k < kws.length; k++) {
            var kw = norm(kws[k]);
            var ks = score(kw, q);
            if (ks === 100) consider(850, kw, "keyword");
            else if (ks === 80) consider(550, kw, "keyword");
            else if (ks === 60) consider(300, kw, "keyword");
            else if (ks === 40) consider(200, kw, "keyword");
        }

        var g = score(norm(app.genericName), q);
        if (g === 100) consider(500, "", "generic");
        else if (g === 80) consider(450, "", "generic");
        else if (g === 60) consider(400, "", "generic");

        if (best.score > 0)
            out.push({ app: app, score: best.score, alias: best.alias, field: best.field });
    }
    out.sort(function (x, y) {
        if (y.score !== x.score)
            return y.score - x.score;
        var lx = (x.app.name || "").length, ly = (y.app.name || "").length;
        if (lx !== ly)
            return lx - ly;
        return norm(x.app.name) < norm(y.app.name) ? -1 : 1;
    });
    return out;
}

// Aliases whose targets are not apps: an exact match runs the target as a
// command. Returns [{ alias, command }].
function commandAliases(apps, query, aliases) {
    var q = norm(query);
    var list = (aliases || {})[q] || [];
    return list.filter(function (t) {
        return !apps.some(function (app) { return targets(app, t); });
    }).map(function (t) { return { alias: q, command: t }; });
}

// "yazi · files" when an alias or keyword matched, else the name.
function label(name, alias) {
    return alias ? name + " · " + alias : name;
}

// Seconds ago -> "now", "4m", "2h", "2 days ago".
function age(seconds) {
    var s = Math.max(0, Math.round(seconds));
    if (s < 60) return "now";
    if (s < 3600) return Math.floor(s / 60) + "m";
    if (s < 86400) return Math.floor(s / 3600) + "h";
    var d = Math.floor(s / 86400);
    if (d === 1) return "yesterday";
    if (d < 60) return d + " days ago";
    return Math.floor(d / 30) + " months ago";
}

// Looks like arithmetic worth showing a result for without the = prefix.
function looksLikeMath(q) {
    var s = q || "";
    if (!/^\s*[-+(]?\s*\d/.test(s))
        return false;
    // 2+2, 3 * (4 - 1), 2^10, 15%
    if (/[\d)]\s*[-+*\/^%×÷]\s*[\d(.]/.test(s))
        return true;
    // 10 kg to lb, 30 eur in huf
    return /^\s*[-+]?[\d.,]+\s*[a-z°$€£]+\s+(to|in)\s+\S/i.test(s);
}
