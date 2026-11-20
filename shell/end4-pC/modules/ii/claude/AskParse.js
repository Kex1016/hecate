.pragma library

// Pure helpers for the ask panel (Mod+/), kept free of QML types so they can
// be unit-checked with node (tests/askParse.test.mjs).

// Fences whose contents are commands the gold button may run. Anything else
// (nix, json, a diff) is shown as code but can only be copied.
var SHELL_LANGS = ["", "sh", "bash", "shell", "console", "zsh", "fish", "nu", "nushell", "terminal", "shell-session", "bash-session"];

// Text and code blocks out of a Markdown answer. Works on a partial answer
// too: an unclosed fence is a code block still being written (`open`).
//   [{ kind: "text", text }, { kind: "code", lang, lines, runnable, open }]
function blocks(markdown) {
    var out = [];
    var lines = String(markdown || "").replace(/\r\n?/g, "\n").split("\n");
    var text = [];
    var code = null;
    function flushText() {
        var joined = text.join("\n");
        text = [];
        // paragraphs: blank lines split them
        var paras = joined.split(/\n[ \t]*\n+/);
        for (var i = 0; i < paras.length; i++) {
            var p = paras[i].replace(/^\n+|\s+$/g, "");
            if (p.length > 0)
                out.push({ kind: "text", text: p });
        }
    }
    for (var i = 0; i < lines.length; i++) {
        var line = lines[i];
        var fence = /^\s*(```+|~~~+)\s*([\w+-]*)/.exec(line);
        if (code === null && fence) {
            flushText();
            code = { kind: "code", fence: fence[1], lang: fence[2].toLowerCase(), lines: [], open: true };
            continue;
        }
        if (code !== null && /^\s*(```+|~~~+)\s*$/.test(line) && line.trim().indexOf(code.fence) === 0) {
            code.open = false;
            out.push(finishCode(code));
            code = null;
            continue;
        }
        if (code !== null)
            code.lines.push(line);
        else
            text.push(line);
    }
    if (code !== null)
        out.push(finishCode(code));
    else
        flushText();
    // runnable blocks are numbered: the gold button runs `command` n
    var n = 0;
    for (var j = 0; j < out.length; j++)
        out[j].command = out[j].kind === "code" && out[j].runnable ? n++ : -1;
    return out;
}

function finishCode(code) {
    var lang = code.lang;
    var runnable = SHELL_LANGS.indexOf(lang) >= 0;
    var lines = code.lines.slice();
    // drop blank lines at either end
    while (lines.length > 0 && lines[0].trim() === "") lines.shift();
    while (lines.length > 0 && lines[lines.length - 1].trim() === "") lines.pop();
    // a console transcript: keep only the commands, without their prompt
    if (lang === "console" || lang === "shell-session" || lang === "bash-session" || lang === "terminal") {
        var cmds = [];
        for (var i = 0; i < lines.length; i++) {
            var m = /^\s*[$#%>]\s+(.*)$/.exec(lines[i]);
            if (m) cmds.push(m[1]);
        }
        if (cmds.length > 0) lines = cmds;
    }
    return { kind: "code", lang: lang, lines: lines, runnable: runnable && lines.length > 0, open: code.open };
}

// The command a code block runs, as one script.
function script(block) {
    return block && block.lines ? block.lines.join("\n") : "";
}

function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// A text block as Qt StyledText: `code` in iris, **bold**, list markers kept.
function styled(text, iris) {
    var parts = String(text).split("`");
    var html = "";
    for (var i = 0; i < parts.length; i++) {
        var p = escapeHtml(parts[i]);
        // an odd part is inside backticks, unless it is the unclosed tail
        if (i % 2 === 1 && i < parts.length - 1) {
            html += "<font color=\"" + iris + "\">" + p + "</font>";
        } else {
            if (i % 2 === 1) p = "`" + p;
            p = p.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>").replace(/__([^_]+)__/g, "<b>$1</b>");
            p = p.replace(/^#{1,6}\s+/gm, "");
            html += p;
        }
    }
    return html.replace(/\n/g, "<br>");
}

function plural(n, one, many) {
    return n + " " + (n === 1 ? one : many);
}

function lineCount(text) {
    var t = String(text || "").replace(/\n+$/, "");
    return t.length === 0 ? 0 : t.split("\n").length;
}

// Where a window title points, if it reads like a terminal's working
// directory: "~/src/cakeos", "cakes@universe: ~/src", "nvim /etc/nixos".
function pathFromTitle(title) {
    var m = /(?:^|[\s:])((?:~|\/)[^\s:]*)/.exec(String(title || ""));
    return m ? m[1] : "";
}

// The footer's list of what goes (or went) with a question:
// "window title + 6 lines", "520×300 region + screenshot", "question only".
// `chips` are the chips that are on: { id, lines, image, width, height }.
function summary(chips) {
    var parts = [];
    for (var i = 0; i < chips.length; i++) {
        var c = chips[i];
        switch (c.id) {
        case "window": parts.push("window title"); break;
        case "selection": parts.push(plural(c.lines || 0, "line", "lines")); break;
        case "screenshot": parts.push("screenshot"); break;
        case "region": parts.push((c.width && c.height ? c.width + "×" + c.height + " " : "") + "region"); break;
        case "clipboard": parts.push(c.image ? "clipboard image" : "clipboard"); break;
        default: parts.push(c.id);
        }
    }
    return parts.length > 0 ? parts.join(" + ") : "question only";
}

// The text half of the message: the chosen context, then the question.
// `ctx`: { window: { appId, title }, selection, clipboard, screenshot, region }
// with only the parts the chips let through.
function prompt(question, ctx) {
    var out = [];
    var c = ctx || {};
    if (c.window)
        out.push("Focused window: " + (c.window.appId || "unknown app") + (c.window.title ? " - " + c.window.title : ""));
    if (c.selection)
        out.push("Selected text in that window:\n<selection>\n" + c.selection + "\n</selection>");
    if (c.clipboard)
        out.push("Clipboard:\n<clipboard>\n" + c.clipboard + "\n</clipboard>");
    if (c.screenshot)
        out.push("A screenshot of the screen is attached.");
    if (c.region)
        out.push("A " + c.region.width + "×" + c.region.height + " region the user selected on screen is attached.");
    if (c.clipboardImage)
        out.push("An image from the clipboard is attached.");
    var q = String(question || "").trim();
    if (out.length === 0)
        return q;
    return out.join("\n\n") + "\n\nQuestion: " + (q.length > 0 ? q : "What is this?");
}

// One line of `--input-format stream-json` for `claude -p`: a user message
// with the images first (base64 placeholders filled in by the shell script)
// and the text after them. Only the text block is built here.
function textBlock(text) {
    return JSON.stringify({ type: "text", text: String(text) });
}

// Reads one line of `--output-format stream-json`. Returns what changed:
//   { session, delta, full, done, error, model }
function event(line) {
    var e;
    try {
        e = JSON.parse(line);
    } catch (err) {
        return null;
    }
    if (!e || typeof e !== "object")
        return null;
    var r = {};
    if (e.session_id) r.session = e.session_id;
    if (e.type === "system" && e.subtype === "init") {
        r.model = e.model || "";
    } else if (e.type === "stream_event" && e.event) {
        var ev = e.event;
        if (ev.type === "content_block_delta" && ev.delta && ev.delta.type === "text_delta")
            r.delta = ev.delta.text || "";
        else if (ev.type === "content_block_start" && ev.content_block && ev.content_block.type === "text" && ev.content_block.text)
            r.delta = ev.content_block.text;
    } else if (e.type === "assistant" && e.message && e.message.content) {
        var text = "";
        for (var i = 0; i < e.message.content.length; i++) {
            var b = e.message.content[i];
            if (b && b.type === "text") text += b.text;
        }
        r.full = text;
        if (e.error) r.error = String(e.error);
    } else if (e.type === "result") {
        r.done = true;
        if (e.is_error || (e.subtype && e.subtype !== "success")) {
            var msg = typeof e.result === "string" && e.result.length > 0 ? e.result
                : (e.errors && e.errors.length ? e.errors.join("\n") : (e.subtype || "error"));
            r.error = msg;
        } else if (typeof e.result === "string") {
            r.result = e.result;
        }
    }
    return r;
}

// A failure the user can fix, in words: not logged in, rate limited, ...
function explain(error) {
    var e = String(error || "");
    if (/log ?in|not logged|authenticat|credential|api key|oauth|401/i.test(e))
        return "Claude Code is not logged in. Run claude in a terminal and log in, then ask again.";
    if (/rate|limit|429|overloaded|529/i.test(e))
        return "Claude is busy or the usage limit is reached. Try again later.";
    if (/ENOTFOUND|ECONNREFUSED|network|connect|offline|timed? ?out/i.test(e))
        return "Claude could not be reached. Check the network and try again.";
    return "";
}
