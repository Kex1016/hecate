.pragma library

// "/home/cakes/src/cakeos" -> "~/src/cakeos"
function shortPath(cwd, home) {
    cwd = cwd ?? "";
    if (home && home !== "/" && (cwd === home || cwd.startsWith(home + "/")))
        return "~" + cwd.slice(home.length);
    return cwd;
}

// 5s -> "now", 3m -> "3m", 2h -> "2h", 1d -> "1d" (the toast stack's ages)
function age(since, now) {
    const s = Math.max(0, Math.floor((now - since) / 1000));
    if (s < 60) return "now";
    if (s < 3600) return Math.floor(s / 60) + "m";
    if (s < 86400) return Math.floor(s / 3600) + "h";
    return Math.floor(s / 86400) + "d";
}

// "12m" for how long a session has been going
function span(since, now) {
    const s = Math.max(0, Math.floor((now - since) / 1000));
    if (s < 60) return s + "s";
    if (s < 3600) return Math.floor(s / 60) + "m";
    return Math.floor(s / 3600) + "h " + Math.floor((s % 3600) / 60) + "m";
}

// waiting first, then working, then done; newest activity first within each
function order(sessions) {
    const rank = { waiting: 0, working: 1, done: 2, idle: 3 };
    return (sessions ?? []).slice().sort((a, b) =>
        ((rank[a.state] ?? 3) - (rank[b.state] ?? 3)) || ((b.updatedAt ?? 0) - (a.updatedAt ?? 0)));
}

// the first line of a done summary, cut to `max` characters
function firstLine(text, max) {
    const line = String(text ?? "").split("\n")[0].trim();
    return line.length > max ? line.slice(0, max - 1) + "…" : line;
}

// what the request wants to run, as one line for a toast: "nix build"
function shortCommand(request) {
    if (!request) return "";
    const c = String(request.command ?? "").trim();
    if (c.length === 0) return String(request.tool ?? "");
    const words = c.split(/\s+/);
    return words.slice(0, 2).join(" ");
}

// text for a StyledText label: a value from outside must not become markup
function esc(s) {
    return String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// a command for a toast: the whole of it up to `max` characters, and how much was left out
function clip(command, max) {
    const c = String(command ?? "").trim();
    return c.length > max ? { text: c.slice(0, max), more: c.length - max } : { text: c, more: 0 };
}
