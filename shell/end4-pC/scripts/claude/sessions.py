"""What the hook bridge knows: Claude Code sessions, their state, and the permission requests
waiting for an answer.

Pure state: no sockets, no processes, no clock of its own (the bridge passes one in). Every
hook message goes through `Store.handle`; every answer goes through `Store.decide`. What the
shell should hear about collects in `Store.out` and the bridge drains it.

The message formats are in README.md next to this file.
"""

import json
import os
import re
import time

VERSION = 1

ID_RE = re.compile(r"^[0-9a-f]{32}$")

# Tools that need the user's own words (an answer, a plan review): "allow" from a toast
# cannot carry them, so these go straight back to the terminal.
TERMINAL_ONLY_TOOLS = ("AskUserQuestion", "ExitPlanMode")

# Notification types that mean "Claude Code is waiting for you", and for what.
WAITING_NOTIFICATIONS = {
    "permission_prompt": "permission",
    "elicitation_dialog": "input",
    "elicitation_url_dialog": "input",
    "agent_needs_input": "input",
}

# The kind of suggested permission update "always" may apply: an allow rule, as in hook.py.
ALWAYS_TYPE = "addRules"

DESTINATIONS = {
    "session": "for this session",
    "localSettings": "in this repo",
    "projectSettings": "in this repo, for everyone",
    "userSettings": "everywhere",
}

TERMINALS = (
    "kitty", "foot", "footclient", "alacritty", "wezterm-gui", "ghostty", "konsole",
    "gnome-terminal-server", "ptyxis", "kgx", "xterm", "urxvt", "st", "tilix", "terminator",
    "xfce4-terminal", "lxterminal", "qterminal", "contour", "rio", "cool-retro-term", "havoc",
    "wayst", "zutty", "blackbox", "tabby", "warp-terminal", "code", "codium", "zed",
)

# Programs whose first argument says what they do: "nix build", "git push".
SUBCOMMAND_TOOLS = (
    "nix", "git", "cargo", "npm", "pnpm", "yarn", "bun", "deno", "go", "docker", "podman",
    "kubectl", "systemctl", "journalctl", "make", "just", "pip", "pip3", "uv", "gh", "hecate",
    "nixos-rebuild", "home-manager", "apt", "dnf", "pacman", "brew", "flatpak", "rustup",
    "dotnet", "terraform", "cmake", "meson", "qs", "nh", "jj", "hg", "svn", "poetry", "gradle",
)
WRAPPERS = ("sudo", "doas", "env", "time", "nice", "exec", "command", "nohup", "timeout")

MAX_SESSIONS = 200
MAX_REQUESTS = 64
MAX_SUMMARY = 400
MAX_DETAIL = 2000
MAX_CALLS = 64
ENDED_TTL = 30 * 60  # seconds an exited session with a result stays listed
SILENT_TTL = 12 * 3600  # seconds a session whose process is unknown may stay silent

STATE_ORDER = {"waiting": 0, "working": 1, "idle": 2, "done": 3}


def validate(message):
    """True when `message` is a hook message this bridge understands."""
    if not isinstance(message, dict):
        return False
    if message.get("v") != VERSION or message.get("type") != "hook":
        return False
    if not isinstance(message.get("id"), str) or not ID_RE.match(message["id"]):
        return False
    if type(message.get("reply")) is not bool:
        return False
    hook_input = message.get("input")
    if not isinstance(hook_input, dict):
        return False
    session_id = hook_input.get("session_id")
    if not isinstance(session_id, str) or not 0 < len(session_id) <= 200:
        return False
    if not isinstance(hook_input.get("hook_event_name"), str):
        return False
    return "client" not in message or isinstance(message["client"], dict)


def text(value, limit=MAX_DETAIL):
    """`value` as a string of at most `limit` characters ("" for anything that is not one)."""
    if not isinstance(value, str):
        return ""
    return value if len(value) <= limit else value[: limit - 1] + "…"


def number(value):
    return value if type(value) is int and value > 0 else 0


def tilde(path, home):
    """`path` with the home directory written as "~"."""
    if not path or not home:
        return path
    home = home.rstrip("/")
    if path == home:
        return "~"
    if path.startswith(home + "/"):
        return "~" + path[len(home) :]
    return path


def brief(directory, limit=28):
    """A directory short enough for a title: a long one keeps its last two names."""
    if len(directory) <= limit:
        return directory
    names = [name for name in directory.split("/") if name]
    for keep in (2, 1):
        short = "\u2026/" + "/".join(names[-keep:])
        if len(short) <= limit:
            return short
    return "\u2026/" + names[-1][-(limit - 2) :] if names else directory[:limit]


def relative(path, cwd, home):
    """`path` as seen from `cwd`, or with "~" when it lies elsewhere."""
    if cwd and path.startswith(cwd.rstrip("/") + "/"):
        return path[len(cwd.rstrip("/")) + 1 :]
    return tilde(path, home)


def one_line(value, limit):
    return text(" ".join(value.split()), limit) if isinstance(value, str) else ""


def command_subject(command):
    """The few words that name a shell command: "nix build", "git push", "touch"."""
    stripped = command.strip() if isinstance(command, str) else ""
    if not stripped:
        return "a command"
    compound = bool(re.search(r"\n|&&|\|\||[;|`]|\$\(", stripped))
    tokens = stripped.splitlines()[0].split()
    while tokens and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", tokens[0]):
        tokens.pop(0)
    if not tokens:
        return "a command"
    words = [os.path.basename(tokens[0]) or tokens[0]]
    i = 1
    if words[0] in WRAPPERS:
        while i < len(tokens) and (tokens[i].startswith("-") or "=" in tokens[i] or tokens[i].isdigit()):
            i += 1
        if i < len(tokens):
            words.append(os.path.basename(tokens[i]) or tokens[i])
            i += 1
    if words[-1] in SUBCOMMAND_TOOLS and i < len(tokens) and re.match(r"^[a-z][a-z0-9:_-]*$", tokens[i]):
        words.append(tokens[i])
    subject = text(" ".join(words), 40)
    return subject + " …" if compound else subject


def host_of(url):
    match = re.match(r"^[a-z][a-z0-9+.-]*://([^/?#]+)", url if isinstance(url, str) else "")
    if not match:
        return ""
    return match.group(1).rsplit("@", 1)[-1]


def describe(tool, tool_input, cwd="", home=""):
    """How to say a tool call: (verb, subject, detail, activity).

    "<dir> wants to <verb> <subject>" is the toast's line, `detail` the whole command, path
    or URL, and `activity` what the session shows while the call runs."""
    data = tool_input if isinstance(tool_input, dict) else {}
    tool = tool if isinstance(tool, str) else ""
    path = data.get("file_path") or data.get("notebook_path") or data.get("path") or ""
    path = path if isinstance(path, str) else ""
    name = os.path.basename(path.rstrip("/")) or path
    rel = relative(path, cwd, home) if path else ""

    if tool in ("Bash", "PowerShell", "Monitor"):
        command = data.get("command") if isinstance(data.get("command"), str) else ""
        subject = command_subject(command)
        return "run", subject, text(command), "running " + subject
    if tool in ("Edit", "MultiEdit", "NotebookEdit"):
        return "edit", name or "a file", text(rel), "editing " + (rel or "a file")
    if tool == "Write":
        return "write", name or "a file", text(rel), "writing " + (rel or "a file")
    if tool == "Read":
        return "read", name or "a file", text(rel), "reading " + (rel or "a file")
    if tool in ("Glob", "Grep"):
        pattern = one_line(data.get("pattern"), 60) or "files"
        return "search", pattern, text((pattern + " in " + rel) if rel else pattern), "searching " + pattern
    if tool == "WebFetch":
        url = data.get("url") if isinstance(data.get("url"), str) else ""
        host = host_of(url) or "a page"
        return "fetch", host, text(url), "fetching " + host
    if tool == "WebSearch":
        query = one_line(data.get("query"), 200)
        return "search", "the web", query, "searching the web"
    if tool in ("Agent", "Task"):
        kind = one_line(data.get("subagent_type"), 40) or "an agent"
        about = one_line(data.get("description"), 200)
        return "start", kind, about, "running " + kind + (": " + about if about else "")
    if tool == "AskUserQuestion":
        return "ask", "you a question", "", "asking a question"
    if tool == "ExitPlanMode":
        return "leave", "plan mode", text(data.get("plan")), "presenting its plan"
    if tool.startswith("mcp__"):
        parts = tool.split("__", 2)
        subject = (parts[1] + ": " + parts[2]) if len(parts) == 3 else tool
        detail = text(json.dumps(data, sort_keys=True, default=str)) if data else ""
        return "use", text(subject, 60), detail, "using " + text(subject, 60)
    detail = text(json.dumps(data, sort_keys=True, default=str)) if data else ""
    return "use", text(tool, 60) or "a tool", detail, "using " + (text(tool, 60) or "a tool")


def always_entry_ok(entry):
    """True when "always" may apply this suggested permission update (hook.py agrees)."""
    if not isinstance(entry, dict) or entry.get("type") != ALWAYS_TYPE:
        return False
    rules = entry.get("rules")
    if entry.get("behavior") != "allow" or not isinstance(rules, list) or not rules:
        return False
    return all(isinstance(r, dict) and isinstance(r.get("toolName"), str) for r in rules)


def always_choice(suggestions, home=""):
    """(index, label) of the allow rule "always" applies, or (-1, "") when none was offered."""
    if not isinstance(suggestions, list):
        return -1, ""
    for index, entry in enumerate(suggestions):
        if not always_entry_ok(entry):
            continue
        rules = []
        for rule in entry["rules"]:
            content = rule.get("ruleContent")
            if isinstance(content, str) and content.startswith("//"):
                # "//path" is an absolute path in a rule; under the home directory "~/path" says the same
                short = tilde(content[1:], home)
                content = short if short.startswith("~") else content
            rules.append(rule["toolName"] + ("(" + content + ")" if isinstance(content, str) and content else ""))
        where = DESTINATIONS.get(entry.get("destination"), "")
        return index, text((", ".join(rules) + " " + where).strip(), 200)
    return -1, ""


def find_terminal(client):
    """The terminal emulator among the hook's ancestors: {name, pid, windowId, tty}.

    `windowId` is the compositor's id of its window, which only the shell can know: it stays
    empty here and ClaudeSessions.qml fills it in."""
    found = {"name": "", "pid": 0, "windowId": "", "tty": text(client.get("tty"), 64)}
    ancestors = client.get("ancestors")
    if not isinstance(ancestors, list):
        return found
    for item in ancestors[:16]:
        if not isinstance(item, list) or len(item) != 2 or not isinstance(item[1], str):
            continue
        pid, name = number(item[0]), item[1].lstrip(".")
        for terminal in TERMINALS:
            short = terminal[:15]
            # NixOS wraps programs: ".kitty-wrapped", cut to 15 characters like every name here.
            if name == short or (name.startswith(terminal) and "-wrapped".startswith(name[len(terminal) :])):
                found["name"] = "gnome-terminal" if terminal.startswith("gnome-terminal") else terminal
                found["pid"] = pid
                return found
    return found


class Store:
    def __init__(self, home=None, clock=time.time):
        self.home = os.path.expanduser("~") if home is None else home
        self.clock = clock
        self.sessions = {}
        self.requests = {}
        self.out = []  # events for the shell, drained by the bridge
        self.dirty = True

    # ---- output ----------------------------------------------------------------------------

    def emit(self, name, **fields):
        event = {"type": "event", "name": name}
        event.update(fields)
        self.out.append(event)

    def drain(self):
        out, self.out = self.out, []
        return out

    def public_session(self, s):
        return {
            "id": s["id"],
            "cwd": s["cwd"],
            "dir": tilde(s["cwd"], self.home),
            "name": s["title"] or os.path.basename(s["cwd"].rstrip("/")) or s["cwd"] or "claude",
            "state": s["state"],
            "waitingFor": s["waiting_for"] if s["state"] == "waiting" else "",
            "activity": s["activity"] if s["state"] in ("working", "waiting") else "",
            "tool": s["tool"] if s["state"] in ("working", "waiting") else "",
            "summary": s["summary"],
            "error": s["error"],
            "message": s["message"],
            "permissionMode": s["permission_mode"],
            "model": s["model"],
            "startedAt": int(s["started"] * 1000),
            "updatedAt": int(s["updated"] * 1000),
            "turnStartedAt": int(s["turn_started"] * 1000),
            "doneAt": int(s["done"] * 1000),
            "pid": s["pid"],
            "terminal": dict(s["terminal"]),
            "subagents": s["subagents"],
            "ended": s["ended"] > 0,
            "requests": [r["id"] for r in self.requests.values() if r["session"] == s["id"]],
        }

    def public_request(self, r):
        s = self.sessions.get(r["session"])
        cwd = s["cwd"] if s else r["cwd"]
        directory = tilde(cwd, self.home)
        return {
            "id": r["id"],
            "sessionId": r["session"],
            "cwd": cwd,
            "dir": directory,
            "name": (s["title"] if s else "") or os.path.basename(cwd.rstrip("/")) or cwd or "claude",
            "tool": r["tool"],
            "verb": r["verb"],
            "subject": r["subject"],
            "title": "%s wants to %s %s" % (brief(directory) or "claude", r["verb"], r["subject"]),
            "detail": r["detail"],
            "input": r["input"],
            "agentType": r["agent_type"],
            "createdAt": int(r["created"] * 1000),
            "canAlways": r["always"] >= 0,
            "alwaysLabel": r["always_label"],
        }

    def snapshot(self):
        sessions = sorted(self.sessions.values(), key=lambda s: (STATE_ORDER.get(s["state"], 9), s["started"], s["id"]))
        return {
            "type": "state",
            "sessions": [self.public_session(s) for s in sessions],
            "requests": [self.public_request(r) for r in self.requests.values()],
        }

    def watch_list(self):
        """(request id, transcript path, tool_use_id) for requests whose answer in the
        terminal can be seen in the session's transcript."""
        found = []
        for r in self.requests.values():
            s = self.sessions.get(r["session"])
            if s and r["tool_use_id"] and s["transcript"]:
                found.append((r["id"], s["transcript"], r["tool_use_id"]))
        return found

    # ---- sessions --------------------------------------------------------------------------

    def session(self, hook_input, client, now):
        sid = hook_input["session_id"]
        s = self.sessions.get(sid)
        created = s is None
        if created:
            if len(self.sessions) >= MAX_SESSIONS:
                return None, False
            s = self.sessions[sid] = {
                "id": sid, "cwd": "", "title": "", "state": "idle", "waiting_for": "", "activity": "",
                "tool": "", "summary": "", "error": "", "message": "", "permission_mode": "",
                "model": "", "started": now, "updated": now, "turn_started": 0.0, "done": 0.0,
                "pid": 0, "terminal": {"name": "", "pid": 0, "windowId": "", "tty": ""},
                "subagents": 0, "ended": 0.0, "transcript": "", "calls": {}, "resolved": 0.0,
            }
        s["updated"] = now
        s["ended"] = 0.0  # it speaks, so it lives
        cwd = hook_input.get("cwd")
        if isinstance(cwd, str) and cwd and not hook_input.get("agent_id"):
            s["cwd"] = text(cwd, 1024)
        elif isinstance(cwd, str) and cwd and not s["cwd"]:
            s["cwd"] = text(cwd, 1024)
        if isinstance(hook_input.get("permission_mode"), str):
            s["permission_mode"] = text(hook_input["permission_mode"], 40)
        if isinstance(hook_input.get("transcript_path"), str) and not hook_input.get("agent_id"):
            s["transcript"] = text(hook_input["transcript_path"], 2048)
        if isinstance(hook_input.get("session_title"), str):
            s["title"] = one_line(hook_input["session_title"], 80)
        if number(client.get("claude_pid")) > 1:
            s["pid"] = client["claude_pid"]
        terminal = find_terminal(client)
        if terminal["pid"] or not s["terminal"]["pid"]:
            for key, value in terminal.items():
                if value or not s["terminal"][key]:
                    s["terminal"][key] = value
        return s, created

    def event_time(self, client, now):
        stamp = client.get("time")
        if type(stamp) in (int, float) and abs(stamp / 1000.0 - now) < 3600:
            return stamp / 1000.0
        return now

    def session_requests(self, sid, main_only=False):
        return [r for r in self.requests.values() if r["session"] == sid and not (main_only and r["agent_type"])]

    def settle(self, s):
        """After its requests changed: a session waits for permission only while one is pending."""
        if s["state"] == "waiting" and s["waiting_for"] == "permission" and not self.session_requests(s["id"]):
            s["state"] = "working"
            s["waiting_for"] = ""

    def resolve(self, request_id, outcome):
        """Drop a request without an answer from here ("terminal", "handedBack", "expired")."""
        r = self.requests.pop(request_id, None)
        if r is None:
            return False
        s = self.sessions.get(r["session"])
        if s is not None:
            s["resolved"] = self.clock()
            self.settle(s)
        self.emit("permissionResolved", id=request_id, outcome=outcome)
        self.dirty = True
        return True

    def end_session(self, s, now):
        for r in self.session_requests(s["id"]):
            self.resolve(r["id"], "terminal")
        if s["state"] == "done" and (s["summary"] or s["error"]):
            s["ended"] = now  # stays listed while it has a result to show
        else:
            self.sessions.pop(s["id"], None)
        self.emit("sessionEnded", id=s["id"])
        self.dirty = True

    def dismiss(self, session_id):
        s = self.sessions.get(session_id)
        if s is None:
            return False
        for r in self.session_requests(session_id):
            self.resolve(r["id"], "handedBack")
        self.sessions.pop(session_id, None)
        self.dirty = True
        return True

    def tick(self, alive=None):
        """Forget sessions that are gone. `alive(pid)` says whether a process still runs."""
        now = self.clock()
        for s in list(self.sessions.values()):
            if s["ended"]:
                if now - s["ended"] > ENDED_TTL:
                    self.sessions.pop(s["id"], None)
                    self.dirty = True
            elif s["pid"] and alive is not None:
                if not alive(s["pid"]):
                    self.end_session(s, now)
            elif not s["pid"] and now - s["updated"] > SILENT_TTL:
                self.end_session(s, now)

    # ---- hook events -----------------------------------------------------------------------

    def handle(self, message):
        """Apply one validated hook message.

        Returns ("hold", request) when the hook must be kept waiting for an answer,
        ("pass", reason) when it asked for one and gets none, and None when it did not ask."""
        hook_input = message["input"]
        client = message.get("client") or {}
        now = self.clock()
        event = hook_input["hook_event_name"]
        wants_reply = message["reply"] and event == "PermissionRequest"

        s, created = self.session(hook_input, client, now)
        if s is None:
            return ("pass", "too many sessions") if wants_reply else None
        self.dirty = True
        when = self.event_time(client, now)
        call_hash = text(client.get("input_hash"), 64)
        tool_use_id = text(hook_input.get("tool_use_id"), 128)
        if created:
            self.emit("sessionStarted", session=self.public_session(s))

        result = None
        if event == "SessionStart":
            if isinstance(hook_input.get("model"), str):
                s["model"] = text(hook_input["model"], 80)
        elif event == "UserPromptSubmit":
            for r in self.session_requests(s["id"], main_only=True):
                self.resolve(r["id"], "terminal")
            s.update(state="working", waiting_for="", turn_started=now, activity="", tool="", error="")
        elif event == "PreToolUse":
            if call_hash and tool_use_id:
                s["calls"][call_hash] = tool_use_id
                while len(s["calls"]) > MAX_CALLS:
                    s["calls"].pop(next(iter(s["calls"])))
                for r in self.session_requests(s["id"]):
                    if r["input_hash"] == call_hash and not r["tool_use_id"]:
                        r["tool_use_id"] = tool_use_id
            if when >= s["done"]:
                self.activity(s, hook_input, now)
        elif event == "PermissionRequest":
            result = self.permission_request(s, message, wants_reply, now)
        elif event in ("PostToolUse", "PostToolUseFailure", "PermissionDenied"):
            for r in self.session_requests(s["id"]):
                same_call = tool_use_id and r["tool_use_id"] == tool_use_id
                if same_call or (call_hash and r["input_hash"] == call_hash):
                    self.resolve(r["id"], "terminal")
            if when >= s["done"] and not self.session_requests(s["id"]):
                if s["state"] in ("waiting", "idle"):
                    s.update(state="working", waiting_for="")
                    s["turn_started"] = s["turn_started"] or now
        elif event == "Notification":
            kind = text(hook_input.get("notification_type"), 64)
            s["message"] = one_line(hook_input.get("message"), 300)
            waits_for = WAITING_NOTIFICATIONS.get(kind)
            stale = kind == "permission_prompt" and when < s["resolved"]
            if waits_for and not stale and s["state"] != "done":
                s.update(state="waiting", waiting_for=waits_for)
                s["turn_started"] = s["turn_started"] or now
            self.emit("notified", session=self.public_session(s), notificationType=kind, message=s["message"])
        elif event in ("Stop", "StopFailure"):
            for r in self.session_requests(s["id"], main_only=True):
                self.resolve(r["id"], "terminal")
            s.update(state="done", waiting_for="", activity="", tool="", turn_started=0.0, done=max(now, when))
            s["summary"] = one_line(hook_input.get("last_assistant_message"), MAX_SUMMARY)
            s["error"] = text(hook_input.get("error"), 80) if event == "StopFailure" else ""
            if event == "StopFailure" and not s["error"]:
                s["error"] = "unknown"
            self.emit("sessionDone", session=self.public_session(s))
        elif event == "SubagentStart":
            s["subagents"] += 1
        elif event == "SubagentStop":
            s["subagents"] = max(0, s["subagents"] - 1)
        elif event == "SessionEnd":
            self.end_session(s, now)
        return result

    def activity(self, s, hook_input, now):
        _verb, _subject, _detail, activity = describe(hook_input.get("tool_name"), hook_input.get("tool_input"), s["cwd"], self.home)
        s["activity"] = text(activity, 200)
        s["tool"] = text(hook_input.get("tool_name"), 80)
        if s["state"] != "waiting" or not self.session_requests(s["id"]):
            if s["state"] != "working":
                s["turn_started"] = s["turn_started"] or now
            s.update(state="working", waiting_for="")

    def permission_request(self, s, message, wants_reply, now):
        hook_input = message["input"]
        client = message.get("client") or {}
        tool = text(hook_input.get("tool_name"), 200)
        verb, subject, detail, activity = describe(tool, hook_input.get("tool_input"), s["cwd"], self.home)
        s["activity"] = text(activity, 200)
        s["tool"] = text(tool, 80)
        s["turn_started"] = s["turn_started"] or now
        if tool in TERMINAL_ONLY_TOOLS:
            s.update(state="waiting", waiting_for="input")
            return ("pass", "needs the terminal") if wants_reply else None
        s.update(state="waiting", waiting_for="permission")
        if not wants_reply:
            return None
        if message["id"] in self.requests or len(self.requests) >= MAX_REQUESTS:
            return ("pass", "too many requests")
        call_hash = text(client.get("input_hash"), 64)
        index, label = always_choice(hook_input.get("permission_suggestions"), self.home)
        tool_input = hook_input.get("tool_input")
        r = self.requests[message["id"]] = {
            "id": message["id"],
            "session": s["id"],
            "cwd": s["cwd"],
            "tool": tool,
            "verb": verb,
            "subject": subject,
            "detail": detail,
            "input": tool_input if isinstance(tool_input, dict) else {},
            "agent_type": text(hook_input.get("agent_type"), 80) if hook_input.get("agent_id") else "",
            "created": now,
            "always": index,
            "always_label": label,
            "input_hash": call_hash,
            "tool_use_id": s["calls"].get(call_hash, "") if call_hash else "",
        }
        public = self.public_request(r)
        self.emit("permissionRequested", request=public)
        return ("hold", public)

    # ---- answers ---------------------------------------------------------------------------

    def decide(self, request_id, behavior, scope="once", message="", interrupt=False):
        """Answer a pending request. Returns the line for its hook, or None when there is no
        such request or the answer is not one."""
        if behavior not in ("allow", "deny") or request_id not in self.requests:
            return None
        r = self.requests.pop(request_id)
        wire = {"v": VERSION, "type": "decision", "id": request_id, "behavior": behavior}
        outcome = behavior
        if behavior == "allow":
            if scope == "always" and r["always"] >= 0:
                wire["suggestion"] = r["always"]
                outcome = "allowAlways"
        else:
            wire["message"] = text(message, 500).strip() or "Denied from the desktop."
            if interrupt is True:
                wire["interrupt"] = True
        s = self.sessions.get(r["session"])
        if s is not None:
            s["resolved"] = self.clock()
            self.settle(s)
        self.emit("permissionResolved", id=request_id, outcome=outcome)
        self.dirty = True
        return wire
