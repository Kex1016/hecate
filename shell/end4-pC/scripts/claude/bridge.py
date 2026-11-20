#!/usr/bin/env python3
"""The shell's end of the Claude Code hook bridge.

services/claude/ClaudeSessions.qml runs this and talks to it over stdin and stdout, one JSON
object per line. It listens on a user-only Unix socket for hook.py, keeps the session list
(sessions.py), holds the connection of each permission request open until someone answers,
and sends each request as a desktop notification with allow / deny / look actions.

An answer comes from two places only: a "decide" line on stdin (the shell), or the action of
a notification this process sent. Nothing that connects to the socket can answer, and nothing
is ever answered by default: when this process stops, every waiting hook is passed back to
its terminal.

    bridge.py [--socket PATH] [--no-notify] [--notify-send PROGRAM] [--standalone]

`--standalone` keeps running when stdin closes (for running it by hand).
The message formats are in README.md next to this file.
"""

import argparse
import errno
import json
import os
import selectors
import signal
import socket
import stat
import struct
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import sessions  # noqa: E402

VERSION = sessions.VERSION
MAX_LINE = 1 << 20
FIRST_LINE_TIMEOUT = 5.0  # seconds a client has to send its message
MAX_CLIENTS = 256
MAX_TRANSCRIPT_READ = 4 << 20
NOTE_BODY = 300
NOTE_SETTLE = 0.05  # seconds between notify-send showing its notification and taking SIGINT
NOTE_PATIENCE = 2.0  # seconds a notify-send gets to show, and then to go away


class Stop(Exception):
    pass


def default_socket_path():
    runtime = os.environ.get("XDG_RUNTIME_DIR") or "/run/user/%d" % os.getuid()
    return os.path.join(runtime, "hecate-claude", "bridge.sock")


def peer_uid(sock):
    try:
        creds = sock.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i"))
        return struct.unpack("3i", creds)[1]
    except (OSError, struct.error):
        return None


def pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except OSError:
        return True
    return True


def escape_markup(value):
    return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def prepare_socket(path):
    """Bind the listening socket at `path`: directory 0700, socket 0600, both this user's.

    Raises RuntimeError(code, message) when the place is not safe or another bridge lives."""
    uid = os.getuid()
    directory = os.path.dirname(path) or "."
    try:
        os.mkdir(directory, 0o700)
    except FileExistsError:
        pass
    except OSError as error:
        raise RuntimeError("directory", "cannot create %s: %s" % (directory, error))
    info = os.lstat(directory)
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != uid:
        raise RuntimeError("directory", "%s is not a directory owned by this user" % directory)
    if info.st_mode & 0o077:
        os.chmod(directory, 0o700)

    try:
        existing = os.lstat(path)
    except FileNotFoundError:
        existing = None
    if existing is not None:
        if not stat.S_ISSOCK(existing.st_mode) or existing.st_uid != uid:
            raise RuntimeError("socket", "%s exists and is not this user's socket" % path)
        probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        probe.settimeout(1.0)
        try:
            probe.connect(path)
        except OSError:
            os.unlink(path)  # left behind by a bridge that died
        else:
            raise RuntimeError("running", "another bridge is already listening on %s" % path)
        finally:
            probe.close()

    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    old_umask = os.umask(0o177)
    try:
        server.bind(path)
    except OSError as error:
        server.close()
        raise RuntimeError("socket", "cannot bind %s: %s" % (path, error))
    finally:
        os.umask(old_umask)
    os.chmod(path, 0o600)
    server.listen(64)
    server.setblocking(False)
    return server


class Client:
    def __init__(self, sock, now):
        self.sock = sock
        self.buf = b""
        self.deadline = now + FIRST_LINE_TIMEOUT
        self.request = None  # id of the request this connection waits on


class Note:
    """One notify-send process: the desktop notification of a permission request."""

    def __init__(self, request_id, session_id, proc):
        self.request = request_id
        self.session = session_id
        self.proc = proc
        self.buf = b""
        self.shown = 0.0  # when it printed the notification's id
        self.closing = 0.0  # when the notification was to be withdrawn
        self.signalled = 0.0  # when it was told to withdraw it


class Bridge:
    def __init__(self, path, out=None, notify_send="notify-send", notify=True, standalone=False, clock=time.time):
        self.path = path
        self.out = sys.stdout if out is None else out
        self.notify_send = notify_send
        self.notify = notify
        self.standalone = standalone
        self.clock = clock
        self.store = sessions.Store(clock=clock)
        self.selector = selectors.DefaultSelector()
        self.server = None
        self.inode = None
        self.clients = {}  # socket -> Client
        self.held = {}  # request id -> Client
        self.notes = {}  # request id -> Note
        self.dead_notes = []
        self.watch = {}  # request id -> [path, offset, partial line]
        self.stdin_buf = b""
        self.stdin_open = True
        self.notify_broken = False
        self.last_tick = 0.0

    # ---- output to the shell ---------------------------------------------------------------

    def say(self, message):
        try:
            self.out.write(json.dumps(message, separators=(",", ":"), default=str) + "\n")
            self.out.flush()
        except (OSError, ValueError):
            if not self.standalone:
                raise Stop()

    def flush(self):
        """Tidy up after requests that are no longer pending, then tell the shell what changed:
        the new state first, then the events that led to it."""
        events = self.store.drain()
        for event in events:
            if event["name"] == "permissionResolved":
                request_id = event["id"]
                client = self.held.pop(request_id, None)
                if client is not None:  # resolved without an answer from here
                    self.send(client, {"v": VERSION, "type": "pass", "id": request_id, "reason": event["outcome"]})
                    self.drop(client)
                self.close_note(request_id)
                self.watch.pop(request_id, None)
        if self.store.dirty:
            self.store.dirty = False
            self.say(self.store.snapshot())
        for event in events:
            self.say(event)

    # ---- hook clients ----------------------------------------------------------------------

    def accept(self, _key, _mask):
        while True:
            try:
                sock, _addr = self.server.accept()
            except (BlockingIOError, InterruptedError):
                return
            except OSError:
                return
            if peer_uid(sock) != os.getuid() or len(self.clients) >= MAX_CLIENTS:
                sock.close()
                continue
            sock.setblocking(False)
            client = Client(sock, time.monotonic())
            self.clients[sock] = client
            self.selector.register(sock, selectors.EVENT_READ, self.read_client)

    def drop(self, client):
        if self.clients.pop(client.sock, None) is None:
            return
        try:
            self.selector.unregister(client.sock)
        except (KeyError, ValueError):
            pass
        try:
            client.sock.close()
        except OSError:
            pass
        if client.request is not None and self.held.get(client.request) is client:
            # The hook went away: answered or cancelled in the terminal, or its time ran out.
            del self.held[client.request]
            self.store.resolve(client.request, "terminal")

    def send(self, client, message):
        data = json.dumps(message, separators=(",", ":")).encode("utf-8") + b"\n"
        try:
            client.sock.setblocking(True)
            client.sock.settimeout(1.0)
            client.sock.sendall(data)
            return True
        except OSError:
            return False
        finally:
            try:
                client.sock.setblocking(False)
            except OSError:
                pass

    @staticmethod
    def is_farewell(chunk, request_id):
        try:
            message = json.loads(chunk.split(b"\n", 1)[0].decode("utf-8"))
        except (ValueError, UnicodeDecodeError):
            return False
        return isinstance(message, dict) and message.get("v") == VERSION and message.get("type") == "expired" and message.get("id") == request_id

    def read_client(self, key, _mask):
        client = self.clients.get(key.fileobj)
        if client is None:
            return
        try:
            chunk = client.sock.recv(65536)
        except (BlockingIOError, InterruptedError):
            return
        except OSError:
            chunk = b""
        if client.request is not None:
            # A waiting hook has one thing to say, that its wait ran out. That or anything
            # else, or closing, ends the connection and the request with it.
            if chunk and self.is_farewell(chunk, client.request):
                request_id, client.request = client.request, None
                self.held.pop(request_id, None)
                self.store.resolve(request_id, "expired")
            self.drop(client)
            return
        if not chunk:
            self.drop(client)
            return
        client.buf += chunk
        if b"\n" not in client.buf:
            if len(client.buf) > MAX_LINE:
                self.drop(client)
            return
        line, _rest = client.buf.split(b"\n", 1)
        client.buf = b""
        try:
            message = json.loads(line.decode("utf-8"))
        except (ValueError, UnicodeDecodeError):
            message = None
        if not sessions.validate(message):
            self.drop(client)
            return
        result = self.store.handle(message)
        if result is None:
            self.drop(client)
            return
        kind, payload = result
        if kind == "hold":
            request_id = message["id"]
            client.request = request_id
            client.deadline = None
            self.held[request_id] = client
            if not self.send(client, {"v": VERSION, "type": "pending", "id": request_id}):
                self.drop(client)
                return
            self.open_note(payload)
        else:
            self.send(client, {"v": VERSION, "type": "pass", "id": message["id"], "reason": payload})
            self.drop(client)

    # ---- answers ---------------------------------------------------------------------------

    def decide(self, request_id, behavior, scope="once", message="", interrupt=False):
        client = self.held.get(request_id)
        wire = self.store.decide(request_id, behavior, scope, message, interrupt)
        if wire is None:
            return False
        self.held.pop(request_id, None)
        if client is not None:
            client.request = None
            self.send(client, wire)
            self.drop(client)
        return True

    def command(self, message):
        """One line from the shell."""
        if not isinstance(message, dict):
            return
        kind = message.get("type")
        request_id = message.get("id")
        if kind == "decide" and isinstance(request_id, str):
            behavior = message.get("behavior")
            if behavior == "allow":
                self.decide(request_id, "allow", "always" if message.get("scope") == "always" else "once")
            elif behavior == "deny":
                text = message.get("message")
                self.decide(request_id, "deny", message=text if isinstance(text, str) else "", interrupt=message.get("interrupt") is True)
        elif kind == "pass" and isinstance(request_id, str):
            self.store.resolve(request_id, "handedBack")
        elif kind == "dismiss" and isinstance(message.get("session"), str):
            self.store.dismiss(message["session"])
        elif kind == "config":
            if type(message.get("notifications")) is bool:
                self.notify = message["notifications"]
                if not self.notify:
                    for request_id in list(self.notes):
                        self.close_note(request_id)

    def read_stdin(self, _key, _mask):
        try:
            chunk = os.read(0, 65536)
        except (BlockingIOError, InterruptedError):
            return
        except OSError:
            chunk = b""
        if not chunk:
            self.selector.unregister(0)
            self.stdin_open = False
            if not self.standalone:
                raise Stop()
            return
        self.stdin_buf += chunk
        if len(self.stdin_buf) > MAX_LINE and b"\n" not in self.stdin_buf:
            self.stdin_buf = b""
        while b"\n" in self.stdin_buf:
            line, self.stdin_buf = self.stdin_buf.split(b"\n", 1)
            try:
                self.command(json.loads(line.decode("utf-8")))
            except (ValueError, UnicodeDecodeError):
                continue

    # ---- desktop notifications -------------------------------------------------------------

    def open_note(self, request):
        if not self.notify or self.notify_broken or request["id"] in self.notes:
            return
        body = escape_markup(sessions.text(request["detail"], NOTE_BODY))
        command = [
            self.notify_send,
            "--app-name=claude",
            "--category=permission",
            "--urgency=normal",
            "--expire-time=0",
            "--wait",
            "--print-id",
            "--hint=string:x-hecate-claude-request:" + request["id"],
            "--hint=string:x-hecate-claude-session:" + request["sessionId"],
            "--action=allow=allow",
            "--action=deny=deny",
            "--action=look=look",
            "--",
            request["title"],
            body,
        ]
        try:
            proc = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        except OSError as error:
            self.notify_broken = True
            self.say({"type": "log", "message": "notifications are off: cannot run %s: %s" % (self.notify_send, error)})
            return
        os.set_blocking(proc.stdout.fileno(), False)
        note = Note(request["id"], request["sessionId"], proc)
        self.notes[request["id"]] = note
        self.selector.register(proc.stdout, selectors.EVENT_READ, self.read_note)

    def read_note(self, key, _mask):
        note = next((n for n in self.notes.values() if n.proc.stdout is key.fileobj), None)
        if note is None:
            note = next((n for n in self.dead_notes if n.proc.stdout is key.fileobj), None)
        try:
            chunk = os.read(key.fileobj.fileno(), 4096)
        except (BlockingIOError, InterruptedError):
            return
        except (OSError, ValueError):
            chunk = b""
        if note is None or not chunk:
            # notify-send is done: the toast was answered, closed or withdrawn
            try:
                self.selector.unregister(key.fileobj)
            except (KeyError, ValueError):
                pass
            if note is not None:
                if self.notes.get(note.request) is note:
                    del self.notes[note.request]
                    self.dead_notes.append(note)
                self.reap()
            return
        note.buf += chunk
        while b"\n" in note.buf:
            line, note.buf = note.buf.split(b"\n", 1)
            action = line.decode("utf-8", "replace").strip()
            if not note.shown:
                note.shown = time.monotonic()  # the first line is the notification's id
                if action.isdigit():
                    continue
            if note.closing or self.notes.get(note.request) is not note:
                continue  # withdrawn: whatever it says now is not an answer
            if action == "allow":
                self.decide(note.request, "allow")
            elif action == "deny":
                self.decide(note.request, "deny")
            elif action == "look":
                self.say({"type": "event", "name": "look", "session": note.session})

    def close_note(self, request_id):
        """Withdraw a request's notification."""
        note = self.notes.pop(request_id, None)
        if note is None:
            return
        note.closing = time.monotonic()
        self.dead_notes.append(note)
        self.reap()

    def reap(self):
        """Tell the notify-send of each withdrawn notification to close it, and collect the
        ones that are gone.

        notify-send closes its notification on SIGINT, but only once the notification shows;
        before that SIGINT kills it, and a notification already on its way would stay up with
        nobody behind its buttons. It prints the id when it shows, so that is waited for."""
        now = time.monotonic()
        alive = []
        for note in self.dead_notes:
            if note.proc.poll() is not None:
                try:
                    self.selector.unregister(note.proc.stdout)
                except (KeyError, ValueError):
                    pass
                note.proc.stdout.close()
                continue
            alive.append(note)
            if not note.closing:
                continue
            try:
                if note.signalled:
                    if now - note.signalled > NOTE_PATIENCE:
                        note.proc.kill()
                elif (note.shown and now - note.shown >= NOTE_SETTLE) or now - note.closing > NOTE_PATIENCE:
                    note.signalled = now
                    note.proc.send_signal(signal.SIGINT)
            except OSError:
                pass
        self.dead_notes = alive

    def withdrawing(self):
        """Some withdrawn notification is still waiting for its SIGINT."""
        return any(note.closing and not note.signalled for note in self.dead_notes)

    # ---- housekeeping ----------------------------------------------------------------------

    def watch_transcripts(self):
        """A request answered in the terminal leaves the tool's result in the session's
        transcript; when it shows up, the request is no longer ours to answer."""
        for request_id, path, tool_use_id in self.store.watch_list():
            entry = self.watch.get(request_id)
            try:
                info = os.stat(path)
            except OSError:
                continue
            if not stat.S_ISREG(info.st_mode):
                continue
            if entry is None:
                self.watch[request_id] = [path, info.st_size, b""]
                continue
            if info.st_size <= entry[1]:
                continue
            try:
                with open(path, "rb") as f:
                    f.seek(entry[1])
                    data = f.read(MAX_TRANSCRIPT_READ)
            except OSError:
                continue
            entry[1] += len(data)
            lines = (entry[2] + data).split(b"\n")
            entry[2] = lines.pop()[-MAX_LINE:]
            needle = tool_use_id.encode("utf-8")
            if any(needle in line and b'"tool_result"' in line for line in lines):
                self.store.resolve(request_id, "terminal")

    def periodic(self):
        now = time.monotonic()
        for client in list(self.clients.values()):
            if client.deadline is not None and now > client.deadline:
                self.drop(client)
        if self.store.requests:
            self.watch_transcripts()
        if now - self.last_tick >= 5.0:
            self.last_tick = now
            self.store.tick(pid_alive)
        if self.dead_notes:
            self.reap()

    def shutdown(self):
        for request_id, client in list(self.held.items()):
            self.send(client, {"v": VERSION, "type": "pass", "id": request_id, "reason": "bridge stopped"})
            client.request = None
            self.drop(client)
        self.held.clear()
        for request_id in list(self.notes):
            self.close_note(request_id)
        deadline = time.monotonic() + NOTE_PATIENCE + 1.0
        while self.dead_notes and time.monotonic() < deadline:
            for key, mask in self.selector.select(0.02):
                if key.data == self.read_note:
                    key.data(key, mask)
            self.reap()
        for note in self.dead_notes:
            try:
                note.proc.kill()
            except OSError:
                pass
        if self.server is not None:
            self.server.close()
            try:
                if os.lstat(self.path).st_ino == self.inode:
                    os.unlink(self.path)
            except OSError:
                pass

    def run(self):
        try:
            self.server = prepare_socket(self.path)
        except RuntimeError as error:
            code, message = error.args
            self.say({"type": "error", "code": code, "message": message})
            return 3 if code == "running" else 2
        self.inode = os.lstat(self.path).st_ino
        self.selector.register(self.server, selectors.EVENT_READ, self.accept)
        try:
            os.set_blocking(0, False)
            self.selector.register(0, selectors.EVENT_READ, self.read_stdin)
        except (OSError, ValueError, PermissionError):
            self.stdin_open = False
            if not self.standalone:
                self.say({"type": "error", "code": "stdin", "message": "stdin cannot be read"})
                self.shutdown()
                return 2

        def stop(_signum, _frame):
            raise Stop()

        signal.signal(signal.SIGTERM, stop)
        signal.signal(signal.SIGINT, stop)
        signal.signal(signal.SIGHUP, stop)

        try:
            self.say({"type": "hello", "v": VERSION, "socket": self.path})
            self.flush()
            while True:
                timeout = 0.02 if self.withdrawing() else 0.5 if (self.store.requests or self.dead_notes) else 2.0
                for key, mask in self.selector.select(timeout):
                    key.data(key, mask)
                self.periodic()
                self.flush()
        except Stop:
            pass
        finally:
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            signal.signal(signal.SIGINT, signal.SIG_IGN)
            signal.signal(signal.SIGHUP, signal.SIG_IGN)
            self.shutdown()
        return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description="Claude Code hook bridge for the hecate shell")
    parser.add_argument("--socket", default=os.environ.get("HECATE_CLAUDE_SOCKET") or default_socket_path())
    parser.add_argument("--no-notify", action="store_true", help="do not send desktop notifications")
    parser.add_argument("--notify-send", default="notify-send", help="the notify-send program")
    parser.add_argument("--standalone", action="store_true", help="keep running when stdin closes")
    args = parser.parse_args(argv)
    bridge = Bridge(args.socket, notify_send=args.notify_send, notify=not args.no_notify, standalone=args.standalone)
    return bridge.run()


if __name__ == "__main__":
    sys.exit(main())
