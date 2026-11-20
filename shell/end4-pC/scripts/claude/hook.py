#!/usr/bin/env python3
"""Claude Code hook for the hecate shell: reports the event to the shell's bridge and, for a
permission request, waits for the answer.

Claude Code runs this once per hook event with the event's JSON on stdin. The message format
and the rules are in README.md next to this file. The one rule that matters:

    This script prints a decision only when the event is PermissionRequest and the bridge
    sent a well-formed decision for this request. In every other case, whatever went wrong,
    it prints nothing and exits 0, and Claude Code asks in its terminal as usual.

Standard library only, and no imports from this directory: it has to start fast and run with
whatever Python 3 the hook entry names.
"""

import hashlib
import json
import os
import select
import socket
import stat
import struct
import sys
import time

VERSION = 1
MAX_LINE = 1 << 20  # bytes per message, both ways
MAX_STRING = 4096  # longer strings in the event are cut before it is sent
CONNECT_TIMEOUT = 1.0  # seconds
PENDING_TIMEOUT = 2.0  # the bridge must take the request within this
MAX_MESSAGE = 500  # characters of a deny message passed on to Claude

HEADLESS_WAIT = 120.0  # seconds `claude -p` is kept waiting for an answer

SOCKET_ENV = "HECATE_CLAUDE_SOCKET"
# Most seconds to wait for an answer. Default: no limit next to a terminal prompt (it stands
# the whole time), HEADLESS_WAIT for `claude -p`, where nothing else can answer.
WAIT_ENV = "HECATE_CLAUDE_WAIT"
# "0" keeps this session away from the shell altogether (for a program that runs `claude -p`
# in the background and does not want it listed).
SWITCH_ENV = "HECATE_CLAUDE_BRIDGE"

# The only event this script may answer.
ANSWERED_EVENT = "PermissionRequest"

# Suggested permission updates that "always" may apply. Mode changes (acceptEdits, auto,
# bypassPermissions) are never applied from here.
ALWAYS_TYPES = ("addRules", "addDirectories")


def socket_path(environ=None, uid=None):
    """Where the bridge listens."""
    environ = os.environ if environ is None else environ
    override = environ.get(SOCKET_ENV)
    if override:
        return override
    runtime = environ.get("XDG_RUNTIME_DIR") or "/run/user/%d" % (os.getuid() if uid is None else uid)
    return os.path.join(runtime, "hecate-claude", "bridge.sock")


def path_is_ours(path, uid=None):
    """True when `path` is a socket owned by this user, in a directory owned by this user
    that nobody else can write to."""
    uid = os.getuid() if uid is None else uid
    try:
        directory = os.lstat(os.path.dirname(path) or ".")
        sock = os.lstat(path)
    except OSError:
        return False
    if not stat.S_ISDIR(directory.st_mode) or directory.st_uid != uid:
        return False
    if directory.st_mode & 0o022:
        return False
    return stat.S_ISSOCK(sock.st_mode) and sock.st_uid == uid


def peer_uid(sock):
    """The uid of the process at the other end, or None when it cannot be told."""
    try:
        creds = sock.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i"))
        _pid, uid, _gid = struct.unpack("3i", creds)
        return uid
    except (OSError, AttributeError, struct.error):
        return None


def connect(path, timeout=CONNECT_TIMEOUT):
    """A connection to the bridge, or None. Refuses a socket that is not this user's."""
    if not path_is_ours(path):
        return None
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        sock.settimeout(timeout)
        sock.connect(path)
        if peer_uid(sock) != os.getuid():
            sock.close()
            return None
        return sock
    except OSError:
        sock.close()
        return None


def shrink(value, limit=MAX_STRING):
    """A copy of `value` with long strings cut, so one event never floods the socket."""
    if isinstance(value, str):
        return value if len(value) <= limit else value[:limit] + "…"
    if isinstance(value, list):
        return [shrink(v, limit) for v in value[:200]]
    if isinstance(value, dict):
        return {str(k)[:200]: shrink(v, limit) for k, v in list(value.items())[:200]}
    return value


def input_hash(hook_input):
    """Names one tool call: the same for its PreToolUse, PermissionRequest and PostToolUse."""
    if "tool_name" not in hook_input:
        return ""
    canonical = json.dumps(
        [hook_input.get("tool_name"), hook_input.get("tool_input")],
        sort_keys=True,
        separators=(",", ":"),
        default=str,
    )
    return hashlib.sha256(canonical.encode("utf-8", "replace")).hexdigest()[:32]


def read_proc_stat(pid):
    """(parent pid, command name) of a process, or None."""
    try:
        with open("/proc/%d/stat" % pid, "rb") as f:
            data = f.read().decode("utf-8", "replace")
        close = data.rindex(")")
        name = data[data.index("(") + 1 : close]
        ppid = int(data[close + 2 :].split()[1])
        return ppid, name
    except (OSError, ValueError, IndexError):
        return None


def is_headless(pid):
    """True when the Claude Code process `pid` runs with -p / --print: it has no prompt of its
    own, and a permission request nobody answers is denied."""
    try:
        with open("/proc/%d/cmdline" % pid, "rb") as f:
            args = f.read(1 << 16).split(b"\0")
    except OSError:
        return False
    return b"-p" in args[1:] or b"--print" in args[1:]


def describe_client(environ=None):
    """Who is asking: this process, the Claude Code process and its ancestors (the bridge
    finds the terminal emulator among them)."""
    environ = os.environ if environ is None else environ
    claude_pid = 0
    try:
        claude_pid = int(environ.get("CLAUDE_PID", "") or 0)
    except ValueError:
        claude_pid = 0
    if claude_pid <= 1:
        claude_pid = os.getppid()
    ancestors = []
    pid = claude_pid
    for _ in range(16):
        info = read_proc_stat(pid)
        if info is None:
            break
        ppid, name = info
        ancestors.append([pid, name])
        if ppid <= 1:
            break
        pid = ppid
    tty = ""
    try:
        target = os.readlink("/proc/%d/fd/0" % claude_pid)
        if target.startswith("/dev/pts/") or target.startswith("/dev/tty"):
            tty = target
    except OSError:
        pass
    return {
        "pid": os.getpid(),
        "claude_pid": claude_pid,
        "time": int(time.time() * 1000),
        "tty": tty,
        "ancestors": ancestors,
        "headless": is_headless(claude_pid),
    }


def build_message(hook_input, request_id, environ=None):
    """The one line sent to the bridge."""
    event = hook_input.get("hook_event_name")
    slim = dict(hook_input)
    slim.pop("tool_response", None)  # can be huge, and the shell has no use for it
    client = describe_client(environ)
    client["input_hash"] = input_hash(hook_input)
    return {
        "v": VERSION,
        "type": "hook",
        "id": request_id,
        "reply": event == ANSWERED_EVENT,
        "input": shrink(slim),
        "client": client,
    }


def always_entry(hook_input, index):
    """The permission update Claude Code suggested at `index`, if "always" may apply it."""
    suggestions = hook_input.get("permission_suggestions")
    if type(index) is not int or not isinstance(suggestions, list):
        return None
    if index < 0 or index >= len(suggestions):
        return None
    entry = suggestions[index]
    if not isinstance(entry, dict) or entry.get("type") not in ALWAYS_TYPES:
        return None
    if entry["type"] == "addRules":
        rules = entry.get("rules")
        if entry.get("behavior") != "allow" or not isinstance(rules, list) or not rules:
            return None
        for rule in rules:
            if not isinstance(rule, dict) or not isinstance(rule.get("toolName"), str):
                return None
    else:
        directories = entry.get("directories")
        if not isinstance(directories, list) or not directories:
            return None
        if not all(isinstance(d, str) for d in directories):
            return None
    return entry


def build_output(hook_input, request_id, message):
    """What to print for the bridge's `message`, or None for "print nothing".

    None is the answer to everything that is not exactly an allow or a deny of this request.
    """
    if not isinstance(hook_input, dict) or hook_input.get("hook_event_name") != ANSWERED_EVENT:
        return None
    if not isinstance(message, dict):
        return None
    if message.get("v") != VERSION or message.get("type") != "decision":
        return None
    if not isinstance(request_id, str) or not request_id or message.get("id") != request_id:
        return None
    behavior = message.get("behavior")
    if behavior == "allow":
        decision = {"behavior": "allow"}
        if "suggestion" in message:
            entry = always_entry(hook_input, message["suggestion"])
            if entry is None:
                return None
            decision["updatedPermissions"] = [entry]
    elif behavior == "deny":
        decision = {"behavior": "deny"}
        text = message.get("message")
        if isinstance(text, str) and text.strip():
            decision["message"] = text.strip()[:MAX_MESSAGE]
        if message.get("interrupt") is True:
            decision["interrupt"] = True
    else:
        return None
    return {"hookSpecificOutput": {"hookEventName": ANSWERED_EVENT, "decision": decision}}


def parse_line(line):
    """One message from the bridge, or None when it is not a JSON object."""
    try:
        message = json.loads(line.decode("utf-8"))
    except (ValueError, UnicodeDecodeError):
        return None
    return message if isinstance(message, dict) else None


def wait_for_decision(sock, request_id, max_wait=None, out_fd=1, pending_timeout=PENDING_TIMEOUT):
    """The bridge's decision message for `request_id`, or None.

    None when the bridge does not take the request in time, passes it back, closes the
    connection, sends anything unexpected, `max_wait` runs out, or Claude Code stops
    listening to this hook (its end of our stdout is closed)."""
    start = time.monotonic()
    deadline = None if max_wait is None else start + max_wait
    pending_deadline = start + pending_timeout
    pending = False
    buf = b""
    poller = select.poll()
    sock.setblocking(False)
    poller.register(sock.fileno(), select.POLLIN)
    try:
        poller.register(out_fd, 0)  # hang-ups and errors are reported whatever is asked for
    except (OSError, ValueError):
        out_fd = -1
    while True:
        now = time.monotonic()
        limit = deadline
        if not pending:
            limit = pending_deadline if limit is None else min(limit, pending_deadline)
        if limit is not None and now >= limit:
            return None
        timeout = 1.0 if limit is None else min(1.0, limit - now)
        try:
            events = poller.poll(max(1, int(timeout * 1000)))
        except InterruptedError:
            continue
        for fd, mask in events:
            if fd == out_fd:
                if mask & (select.POLLERR | select.POLLHUP | select.POLLNVAL):
                    return None
                continue
            try:
                chunk = sock.recv(65536)
            except (BlockingIOError, InterruptedError):
                continue
            except OSError:
                return None
            if not chunk:
                return None
            buf += chunk
            if len(buf) > MAX_LINE:
                return None
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                message = parse_line(line)
                if message is None or message.get("v") != VERSION or message.get("id") != request_id:
                    return None
                kind = message.get("type")
                if kind == "pending":
                    pending = True
                elif kind == "decision" and pending:
                    return message
                else:
                    return None


def run(stdin, stdout, environ=None):
    """Handle one hook event. Returns the text to print, "" for no decision."""
    environ = os.environ if environ is None else environ
    if environ.get(SWITCH_ENV, "").strip().lower() in ("0", "off", "false", "no"):
        return ""
    try:
        hook_input = json.loads(stdin.read())
    except (ValueError, UnicodeDecodeError):
        return ""
    if not isinstance(hook_input, dict) or not isinstance(hook_input.get("hook_event_name"), str):
        return ""

    request_id = os.urandom(16).hex()
    message = build_message(hook_input, request_id, environ)
    line = json.dumps(message, separators=(",", ":"), default=str).encode("utf-8") + b"\n"
    if len(line) > MAX_LINE:
        return ""

    sock = connect(socket_path(environ))
    if sock is None:
        return ""
    try:
        sock.sendall(line)
        if not message["reply"]:
            return ""
        max_wait = HEADLESS_WAIT if message["client"].get("headless") else None
        try:
            if environ.get(WAIT_ENV):
                max_wait = max(0.0, float(environ[WAIT_ENV]))
        except ValueError:
            pass
        try:
            out_fd = stdout.fileno()
        except (AttributeError, OSError, ValueError):
            out_fd = -1
        answer = wait_for_decision(sock, request_id, max_wait, out_fd)
        output = build_output(hook_input, request_id, answer)
        return "" if output is None else json.dumps(output, separators=(",", ":"))
    finally:
        try:
            sock.close()
        except OSError:
            pass


def main():
    text = ""
    try:
        text = run(sys.stdin.buffer, sys.stdout)
    except BaseException:  # noqa: BLE001 - nothing may turn into an answer or an error exit
        text = ""
    if text:
        try:
            sys.stdout.write(text + "\n")
            sys.stdout.flush()
        except (OSError, ValueError):
            pass
    os._exit(0)


if __name__ == "__main__":
    main()
