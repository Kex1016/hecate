pragma Singleton
pragma ComponentBehavior: Bound

import qs.services
import qs.modules.common
import qs.modules.common.functions
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../../modules/ii/claude/AskParse.js" as Ask

// The ask panel's brain (Mod+/, the launcher's `?`, the region tools' ask).
//
// It runs the local Claude Code CLI on the user's own login, no API key:
//   claude -p --input-format stream-json --output-format stream-json
// with no tools, no MCP servers, no hooks and no CLAUDE.md, in an empty
// working directory of its own. So the only things that leave the machine
// are the question and the context chips that are on when it is sent, and
// the footer lists exactly those. Commands in an answer never run by
// themselves: runCommand() is only called by the gold button (or ctrl enter).
//
// The panel itself is modules/ii/claude/AskPanel.qml.
Singleton {
    id: root

    // ── what the panel shows ───────────────────────────────────────────────
    property bool open: false
    // compose: writing the question; asking: waiting for the first words;
    // answering: words arriving; done; error
    property string phase: "compose"
    readonly property bool busy: phase === "asking" || phase === "answering"
    // Claude Code found on this machine (the opt-in: nothing runs without it)
    property bool available: false
    property bool checked: false
    property string claudePath: ""

    property string question: ""
    property string answer: ""
    readonly property var blocks: Ask.blocks(root.answer)
    readonly property var commands: root.blocks.filter(b => b.kind === "code" && b.runnable)
    property int commandIndex: 0
    readonly property var command: root.commands[Math.min(root.commandIndex, root.commands.length - 1)] ?? null
    property string error: ""
    readonly property string errorHint: Ask.explain(root.error)
    property string sessionId: ""
    property string model: ""

    // The context chips, in board order:
    //   { id, label, on, lines?, image?, width?, height? }
    property var chips: []
    readonly property var chipsOn: root.chips.filter(c => c.on)
    // what the next question would take with it, and what the last one took
    readonly property string pendingSummary: Ask.summary(root.chipsOn)
    property string sentSummary: ""

    // What the panel saw when it opened. Only what a chip lets through is sent.
    //   window: { appId, title }, selection, clipboard, clipboardImage (path),
    //   screenshot (path), region: { path, width, height }
    property var ctx: ({})
    property var screen: null

    // where a command runs: the focused terminal when there is one
    readonly property string terminal: root.terminalCommand()
    readonly property string terminalName: root.terminal.split(/\s+/)[0].split("/").pop()
    readonly property string workdir: Ask.pathFromTitle(root.ctx.window?.title ?? "")

    readonly property string runDir: `${Quickshell.env("XDG_RUNTIME_DIR") || "/tmp"}/hecate-claude-ask`
    readonly property string askDir: FileUtils.trimFileProtocol(`${Directories.state}/user/claude-ask`)
    readonly property string regionPath: `${root.runDir}/region.png`

    // the panel moves focus to the question field
    signal focusPrompt()

    readonly property string systemPrompt: [
        "You answer one quick question asked from the ask panel of hecate, a Wayland desktop shell on Linux (often NixOS, through CakeOS).",
        "You have no tools: you cannot read files, open pages or run anything, and you must never claim to have.",
        "The message holds only what the user chose to share: maybe the focused window's name, a text selection, the clipboard, or a screenshot or screen region.",
        "Answer briefly and directly, in plain prose. Use Markdown only for `inline code` and fenced code blocks.",
        "Put a command the user should run in its own ```sh block with nothing else in it; they run it in a terminal by pressing a button, so prefer commands that are safe to run as they are and say when one changes the system.",
        "Commands run in bash, in the directory the window shows when there is one."
    ].join(" ")

    // ── entry points ───────────────────────────────────────────────────────

    // Mod+/: about what is in front of you.
    function toggle() {
        if (root.open || root.capturing)
            root.close();
        else
            root.openWith({ kind: "desktop" });
    }

    // Words about the calendar or music go to a card first (AppleCardsPanel sets this to
    // AppleCards.route, which returns true when a card took them); the rest come here.
    property var cardsHook: null
    // a card the launcher opened (AppleCardsPanel) is showing in the ask panel's place
    property bool cardOpen: false

    // Mod+A then `?`: a question with no context at all, sent right away.
    function askFromLauncher(text) {
        launcherDelay.text = String(text ?? "");
        launcherDelay.restart();
    }
    Timer {
        id: launcherDelay
        property string text: ""
        // the launcher closes and gives keyboard focus back first
        interval: 120
        onTriggered: {
            if (typeof root.cardsHook === "function" && root.cardsHook(text))
                return;
            root.openWith({ kind: "bare", question: text });
        }
    }

    // The region tools' ask mode: `path` is a PNG of the region (write it to
    // ClaudeAsk.regionPath). The panel opens with only the region on.
    function askAboutRegion(path, width, height) {
        root.openWith({ kind: "region", region: { path: String(path), width: Math.round(width), height: Math.round(height) } });
    }

    function close() {
        root.capturing = false;
        capture.running = false;
        root.stop();
        root.open = false;
        // screenshots and copies of the clipboard live only while the panel is up
        Quickshell.execDetached(["rm", "-rf", root.runDir]);
    }

    // ── opening: take the context before the panel covers anything ─────────

    property string openKind: "desktop"
    property bool capturing: false
    // the launcher's question, sent once the panel is up
    property string pendingQuestion: ""

    function openWith(how) {
        root.stop();
        root.phase = "compose";
        root.question = "";
        root.answer = "";
        root.error = "";
        root.sessionId = "";
        root.sentSummary = "";
        root.commandIndex = 0;
        root.openKind = how.kind;
        root.pendingQuestion = String(how.question ?? "").trim();
        root.screen = Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0] ?? null;

        const win = how.kind === "bare" ? null : root.focusedWindow();
        root.ctx = { window: win, region: how.region ?? null };
        root.chips = [];

        capture.command = ["bash", "-c", root.captureScript, "claude-ask-capture",
            root.runDir,
            how.kind === "desktop" ? (root.screen?.name ?? "") : "",
            how.kind === "bare" ? "none" : how.kind];
        root.capturing = true;
        finder.running = true;
        capture.running = true;
        captureTimeout.restart();
    }

    function focusedWindow() {
        const w = (WM.windowList ?? []).find(w => w.focused);
        if (w)
            return { appId: w.appId ?? "", title: w.title ?? "" };
        const t = ToplevelManager.activeToplevel;
        if (t)
            return { appId: t.appId ?? "", title: t.title ?? "" };
        return null;
    }

    // Prints one JSON object: the primary selection, the clipboard (text, or
    // an image saved as PNG) and, for Mod+/, a screenshot of the focused
    // output, taken side by side. Nothing here leaves the machine.
    readonly property string captureScript: `
        dir="$1"; output="$2"; kind="$3"
        mkdir -p "$dir" && chmod 700 "$dir"
        rm -f "$dir/selection.txt" "$dir/clipboard.txt" "$dir/clipboard.png" "$dir/screenshot.png"
        if [ "$kind" = "desktop" ] && command -v grim >/dev/null 2>&1; then
            { timeout 2 grim \${output:+-o "$output"} "$dir/screenshot.png" 2>/dev/null || timeout 2 grim "$dir/screenshot.png" 2>/dev/null; } &
        fi
        if [ "$kind" = "desktop" ] || [ "$kind" = "region" ]; then
            timeout 1 wl-paste --primary --no-newline > "$dir/selection.txt" 2>/dev/null
            types=$(timeout 1 wl-paste --list-types 2>/dev/null)
            img=$(printf '%s\\n' "$types" | grep -m1 '^image/')
            if [ -n "$img" ]; then
                if [ "$img" = image/png ]; then
                    timeout 2 wl-paste --type image/png > "$dir/clipboard.png" 2>/dev/null
                else
                    timeout 2 wl-paste --type "$img" 2>/dev/null | magick - "png:$dir/clipboard.png" 2>/dev/null
                fi
            elif [ -n "$types" ]; then
                timeout 1 wl-paste --no-newline > "$dir/clipboard.txt" 2>/dev/null
            fi
        fi
        wait
        exec python3 -c '
import json, os, sys
d = sys.argv[1]
LIMIT = 20000
def text(name):
    try:
        with open(os.path.join(d, name), "rb") as f:
            t = f.read(LIMIT * 4).decode("utf-8", "replace")
    except OSError:
        return None
    return t[:LIMIT] if t.strip() else None
def image(name):
    p = os.path.join(d, name)
    return p if os.path.isfile(p) and os.path.getsize(p) > 0 else None
print(json.dumps({
    "selection": text("selection.txt"),
    "clipboard": text("clipboard.txt"),
    "clipboardImage": image("clipboard.png"),
    "screenshot": image("screenshot.png"),
}))
' "$dir" "$claude"
    `

    Process {
        id: capture
        stdout: StdioCollector {
            id: captureOut
            onStreamFinished: root.captured(captureOut.text)
        }
    }
    // never hold the panel back for long: open with what is there
    Timer {
        id: captureTimeout
        interval: 3000
        onTriggered: if (capture.running) { capture.running = false; root.captured(""); }
    }

    // Where claude is: PATH first, then where its installers put it. Looked
    // up at startup and again each time the panel opens, apart from the
    // capture so a slow clipboard never makes it look missing.
    Process {
        id: finder
        command: ["bash", "-c", `
            c=$(command -v claude 2>/dev/null)
            if [ -z "$c" ]; then
                for p in "$HOME/.local/bin/claude" "$HOME/.claude/local/claude" "$HOME/.nix-profile/bin/claude" "/etc/profiles/per-user/$USER/bin/claude" /run/current-system/sw/bin/claude; do
                    [ -x "$p" ] && { c="$p"; break; }
                done
            fi
            printf '%s' "$c"
        `]
        stdout: StdioCollector {
            id: finderOut
            onStreamFinished: {
                root.claudePath = finderOut.text.trim();
                root.available = root.claudePath.length > 0;
                root.checked = true;
            }
        }
    }

    function captured(text) {
        captureTimeout.stop();
        if (!root.capturing)
            return;
        root.capturing = false;
        let r = {};
        try {
            r = JSON.parse(text);
        } catch (e) {}

        const c = Object.assign({}, root.ctx, {
            selection: r.selection ?? "",
            clipboard: r.clipboard ?? "",
            clipboardImage: r.clipboardImage ?? "",
            screenshot: r.screenshot ?? ""
        });
        root.ctx = c;

        const chips = [];
        const kind = root.openKind;
        if (c.region)
            chips.push({ id: "region", label: `region · ${c.region.width}×${c.region.height}`, on: true, width: c.region.width, height: c.region.height });
        if (c.window) {
            const title = c.window.title.length > 36 ? c.window.title.slice(0, 35) + "…" : c.window.title;
            chips.push({ id: "window", label: `window: ${c.window.appId}${title ? " " + title : ""}`.trim(), on: kind === "desktop" });
        }
        if (c.selection) {
            const n = Ask.lineCount(c.selection);
            chips.push({ id: "selection", label: `selection · ${n} ${n === 1 ? "line" : "lines"}`, on: kind === "desktop", lines: n });
        }
        if (c.screenshot)
            chips.push({ id: "screenshot", label: "screenshot", on: false, image: true });
        if (c.clipboardImage)
            chips.push({ id: "clipboard", label: "clipboard · image", on: false, image: true });
        else if (c.clipboard) {
            const n = Ask.lineCount(c.clipboard);
            chips.push({ id: "clipboard", label: "clipboard", on: false, lines: n });
        }
        root.chips = chips;
        root.open = true;
        root.focusPrompt();
        if (root.pendingQuestion.length > 0) {
            root.ask(root.pendingQuestion);
            root.pendingQuestion = "";
        }
    }

    function toggleChip(id) {
        if (root.busy)
            return;
        root.chips = root.chips.map(c => c.id === id ? Object.assign({}, c, { on: !c.on }) : c);
    }

    // ── asking ─────────────────────────────────────────────────────────────

    // Writes the stream-json user message (images first, then the text) to
    // claude's stdin and replaces itself with claude, so stopping the
    // Process stops claude.
    readonly property string askScript: `
        dir="$1"; text="$2"; shift 2
        imgs=()
        while [ $# -gt 0 ] && [ "$1" != "--" ]; do imgs+=("$1"); shift; done
        shift
        mkdir -p "$dir" && cd "$dir" || exit 1
        message() {
            printf '{"type":"user","message":{"role":"user","content":['
            for f in "\${imgs[@]}"; do
                if command -v magick >/dev/null 2>&1; then
                    data=$(magick "$f" -resize '1568x1568>' png:- 2>/dev/null | base64 -w0)
                fi
                [ -n "$data" ] || data=$(base64 -w0 < "$f")
                [ -n "$data" ] && printf '{"type":"image","source":{"type":"base64","media_type":"image/png","data":"%s"}},' "$data"
                data=
            done
            printf '%s]}}\\n' "$text"
        }
        exec "$@" < <(message)
    `

    function ask(text) {
        if (!root.available || root.busy)
            return;
        const q = String(text ?? "").trim();
        const on = root.chipsOn;
        if (q.length === 0 && on.length === 0)
            return;

        // only what a chip that is on lets through
        const send = {};
        const images = [];
        for (const chip of on) {
            switch (chip.id) {
            case "window": send.window = root.ctx.window; break;
            case "selection": send.selection = root.ctx.selection; break;
            case "clipboard":
                if (root.ctx.clipboardImage) { send.clipboardImage = true; images.push(root.ctx.clipboardImage); }
                else send.clipboard = root.ctx.clipboard;
                break;
            case "screenshot": send.screenshot = true; images.push(root.ctx.screenshot); break;
            case "region": send.region = root.ctx.region; images.push(root.ctx.region.path); break;
            }
        }

        const args = [root.claudePath, "-p",
            "--input-format", "stream-json",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--tools", "",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--settings", JSON.stringify({ disableAllHooks: true }),
            "--system-prompt", root.systemPrompt];
        // a follow-up carries on the same conversation
        if (root.sessionId.length > 0)
            args.push("--resume", root.sessionId);

        root.question = q;
        root.answer = "";
        root.error = "";
        root.commandIndex = 0;
        root.gotDelta = false;
        root.gotResult = false;
        root.sentSummary = Ask.summary(on);
        root.phase = "asking";

        askProc.command = ["bash", "-c", root.askScript, "claude-ask", root.askDir, Ask.textBlock(Ask.prompt(q, send))]
            .concat(images).concat(["--"]).concat(args);
        askProc.running = true;
    }

    function stop() {
        if (askProc.running) {
            askProc.running = false;
            if (root.busy)
                root.phase = root.answer.length > 0 ? "done" : "compose";
        }
    }

    property bool gotDelta: false
    property bool gotResult: false

    Process {
        id: askProc
        environment: ({
            CLAUDE_CODE_DISABLE_CLAUDE_MDS: "1",
            CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1",
            CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY: "1",
            // a question from the panel is not a session for the sessions panel
            HECATE_CLAUDE_BRIDGE: "0"
        })
        stdout: SplitParser {
            onRead: line => root.onEvent(line)
        }
        stderr: StdioCollector {
            id: errOut
        }
        onExited: (code, status) => {
            if (!root.busy)
                return;
            if (!root.gotResult && (code !== 0 || root.answer.length === 0)) {
                const err = errOut.text.trim().split("\n").slice(-3).join("\n");
                root.error = err.length > 0 ? err : `claude stopped (exit ${code}) without an answer.`;
                root.phase = "error";
            } else {
                root.phase = "done";
            }
        }
    }

    function onEvent(line) {
        const e = Ask.event(line);
        if (!e)
            return;
        if (e.session)
            root.sessionId = e.session;
        if (e.model !== undefined)
            root.model = e.model;
        if (e.delta) {
            root.gotDelta = true;
            root.answer += e.delta;
            root.phase = "answering";
        }
        // without partial messages, the whole message at once
        if (e.full !== undefined && !root.gotDelta && e.full.length > 0) {
            root.answer = e.full;
            root.phase = "answering";
        }
        if (e.done) {
            root.gotResult = true;
            if (e.error) {
                root.error = e.error;
                root.phase = "error";
            } else {
                if (root.answer.length === 0 && e.result)
                    root.answer = e.result;
                root.phase = "done";
            }
        }
    }

    // ── what the answer's buttons do ───────────────────────────────────────

    function q(s) {
        return `'${StringUtils.shellSingleQuoteEscape(s)}'`;
    }

    // The focused window's terminal when it is one, else the configured one.
    function terminalCommand() {
        const configured = Config.options?.apps?.terminal ?? "kitty -1";
        const app = (root.ctx.window?.appId ?? "").toLowerCase();
        if (app.length === 0)
            return configured;
        if (configured.split(/\s+/)[0].split("/").pop() === app)
            return configured;
        const known = {
            "kitty": "kitty", "foot": "foot", "footclient": "footclient", "alacritty": "alacritty",
            "ghostty": "ghostty", "com.mitchellh.ghostty": "ghostty", "konsole": "konsole",
            "org.kde.konsole": "konsole", "xterm": "xterm"
        };
        return known[app] ?? configured;
    }

    // Runs a command from the answer in a new terminal window, in the
    // directory the focused window showed. Only the gold button calls this.
    function runCommand(index) {
        const block = root.commands[index ?? root.commandIndex];
        if (!block || block.open)
            return;
        const dir = root.workdir.replace(/^~(?=\/|$)/, Directories.home);
        // the window's directory, else home (never wherever the shell was started)
        const inner = (dir ? `cd ${root.q(dir)} 2>/dev/null || cd; ` : "cd; ") + Ask.script(block) + `\nprintf '\\n'; exec "\${SHELL:-bash}"`;
        Quickshell.execDetached(["bash", "-c", `${root.terminal} -e bash -c "$1"`, "claude-ask-run", inner]);
        root.close();
    }

    function copyCommand(index) {
        const block = root.commands[index ?? root.commandIndex];
        if (block)
            Quickshell.clipboardText = Ask.script(block);
    }

    // Opens the conversation in Claude Code proper (with its usual tools and
    // permission prompts), in a terminal.
    function continueInClaudeCode() {
        if (root.sessionId.length === 0 || !root.available)
            return;
        const inner = `cd ${root.q(root.askDir)} && ${root.q(root.claudePath)} --resume ${root.q(root.sessionId)}; exec "\${SHELL:-bash}"`;
        Quickshell.execDetached(["bash", "-c", `${root.terminal} -e bash -c "$1"`, "claude-ask-continue", inner]);
        root.close();
    }

    function selectCommand(delta) {
        if (root.commands.length > 0)
            root.commandIndex = (root.commandIndex + delta + root.commands.length) % root.commands.length;
    }

    // ── wiring ─────────────────────────────────────────────────────────────

    // The launcher's `?` prefix (LauncherSearch.claudeHook).
    Component.onCompleted: {
        LauncherSearch.claudeHook = text => root.askFromLauncher(text);
        finder.running = true;
    }

    IpcHandler {
        target: "claudeAsk"

        function toggle(): void {
            root.toggle();
        }
        function open(): void {
            if (!root.open)
                root.openWith({ kind: "desktop" });
        }
        function close(): void {
            root.close();
        }
        // like the launcher's `?`: no context, sent right away
        function ask(text: string): void {
            root.askFromLauncher(text);
        }
        // like the region tools: a PNG of a region
        function region(path: string, width: int, height: int): void {
            root.askAboutRegion(path, width, height);
        }
    }
}
