"""bridge.py with real sockets: hook.py on one side, the shell's stdin/stdout on the other."""

import json
import os
import queue
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest

HERE = os.path.dirname(os.path.realpath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)

HOOK = os.path.join(ROOT, "hook.py")
BRIDGE = os.path.join(ROOT, "bridge.py")
FAKE_NOTIFY = os.path.join(HERE, "fake_notify_send.py")

RULE = {"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "nix build:*"}], "behavior": "allow", "destination": "localSettings"}
MODE = {"type": "setMode", "mode": "acceptEdits", "destination": "session"}
ALLOW = {"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}}


class BridgeProcess:
    def __init__(self, test, runtime, notify=True, extra=(), env_extra=None):
        self.runtime = runtime
        self.notes = os.path.join(runtime, "notes")
        os.makedirs(self.notes, exist_ok=True)
        self.path = os.path.join(runtime, "hecate-claude", "bridge.sock")
        env = dict(os.environ, XDG_RUNTIME_DIR=runtime, FAKE_NOTIFY_DIR=self.notes)
        env.pop("HECATE_CLAUDE_SOCKET", None)
        env = dict(env, **(env_extra or {}))
        args = [sys.executable, BRIDGE, "--notify-send", FAKE_NOTIFY] + ([] if notify else ["--no-notify"]) + list(extra)
        self.proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)
        self.lines = queue.Queue()
        self.seen = []
        self.state = None
        threading.Thread(target=self.pump, daemon=True).start()
        test.addCleanup(self.stop)

    def pump(self):
        for raw in self.proc.stdout:
            self.lines.put(json.loads(raw))
        self.lines.put(None)

    def next(self, timeout=10):
        message = self.lines.get(timeout=timeout)
        if message is not None:
            self.seen.append(message)
            if message.get("type") == "state":
                self.state = message
        return message

    def wait(self, predicate, timeout=10):
        """The first message (from now on) that `predicate` accepts."""
        deadline = time.monotonic() + timeout
        while True:
            message = self.next(max(0.01, deadline - time.monotonic()))
            if message is None:
                raise AssertionError("bridge stopped; saw %r" % self.seen[-5:])
            if predicate(message):
                return message

    def event(self, name, timeout=10):
        return self.wait(lambda m: m.get("type") == "event" and m.get("name") == name, timeout)

    def settle(self, predicate, timeout=10):
        """The newest state snapshot once `predicate` accepts it. The bridge writes a state
        before the events that led to it, so after waiting for an event the newest state read
        is the one that goes with it."""
        if self.state is not None and predicate(self.state):
            return self.state
        return self.wait(lambda m: m.get("type") == "state" and predicate(m), timeout)

    def events(self, name):
        return [m for m in self.seen if m.get("type") == "event" and m.get("name") == name]

    def say(self, **message):
        self.proc.stdin.write(json.dumps(message).encode() + b"\n")
        self.proc.stdin.flush()

    def click(self, request_id, action):
        with open(os.path.join(self.notes, request_id + ".action.tmp"), "w") as f:
            f.write(action)
        os.rename(os.path.join(self.notes, request_id + ".action.tmp"), os.path.join(self.notes, request_id + ".action"))

    def note_args(self, request_id, timeout=10):
        path = os.path.join(self.notes, request_id + ".args")
        deadline = time.monotonic() + timeout
        while not os.path.exists(path):
            if time.monotonic() > deadline:
                raise AssertionError("no notification was sent")
            time.sleep(0.01)
        with open(path) as f:
            return json.load(f)

    def note_closed(self, request_id, timeout=10):
        path = os.path.join(self.notes, request_id + ".closed")
        deadline = time.monotonic() + timeout
        while not os.path.exists(path):
            if time.monotonic() > deadline:
                return False
            time.sleep(0.01)
        return True

    def stop(self):
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.proc.stdout.close()


class Case(unittest.TestCase):
    notify = True

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="hcb")
        self.addCleanup(self.tmp.cleanup)
        self.runtime = self.tmp.name
        self.bridge = BridgeProcess(self, self.runtime, notify=self.notify)
        hello = self.bridge.wait(lambda m: m.get("type") in ("hello", "error"))
        self.assertEqual(hello["type"], "hello", hello)
        self.hooks = []
        self.addCleanup(self.kill_hooks)

    def kill_hooks(self):
        for proc in self.hooks:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
            proc.stdout.close()

    def hook_input(self, event, session="s1", **fields):
        data = {"session_id": session, "hook_event_name": event, "cwd": "/home/cakes/src/cakeos", "permission_mode": "default",
                "transcript_path": os.path.join(self.runtime, session + ".jsonl")}
        data.update(fields)
        return data

    def start_hook(self, event, env=None, **fields):
        environ = {"PATH": os.environ.get("PATH", ""), "XDG_RUNTIME_DIR": self.runtime}
        environ.update(env or {})
        proc = subprocess.Popen([sys.executable, HOOK], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=environ)
        proc.stdin.write(json.dumps(self.hook_input(event, **fields)).encode())
        proc.stdin.close()
        self.hooks.append(proc)
        return proc

    def run_hook(self, event, **fields):
        proc = self.start_hook(event, **fields)
        out = proc.stdout.read()
        self.assertEqual(proc.wait(timeout=10), 0)
        return out

    def ask(self, command="nix build .#cakeos", **fields):
        fields.setdefault("tool_name", "Bash")
        fields.setdefault("tool_input", {"command": command})
        fields.setdefault("permission_suggestions", [MODE, RULE])
        proc = self.start_hook("PermissionRequest", **fields)
        request = self.bridge.event("permissionRequested")["request"]
        return proc, request

    def finish(self, proc, timeout=10):
        out = proc.stdout.read()
        self.assertEqual(proc.wait(timeout=timeout), 0)
        return json.loads(out) if out.strip() else None

    def resolved(self, request):
        return self.bridge.wait(lambda m: m.get("name") == "permissionResolved" and m.get("id") == request["id"])["outcome"]


class Answers(Case):
    def test_allow_from_the_shell(self):
        proc, request = self.ask()
        self.assertEqual(request["title"], "/home/cakes/src/cakeos wants to run nix build")
        state = self.bridge.settle(lambda s: s["requests"])
        self.assertEqual(state["sessions"][0]["state"], "waiting")
        self.bridge.say(type="decide", id=request["id"], behavior="allow", scope="once")
        self.assertEqual(self.finish(proc), ALLOW)
        self.assertEqual(self.resolved(request), "allow")
        state = self.bridge.settle(lambda s: not s["requests"])
        self.assertEqual(state["sessions"][0]["state"], "working")
        self.assertTrue(self.bridge.note_closed(request["id"]))  # its toast is withdrawn

    def test_allow_always_from_the_shell(self):
        proc, request = self.ask()
        self.assertEqual((request["canAlways"], request["alwaysLabel"]), (True, "Bash(nix build:*) in this repo"))
        self.bridge.say(type="decide", id=request["id"], behavior="allow", scope="always")
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"], {"behavior": "allow", "updatedPermissions": [RULE]})
        self.assertEqual(self.resolved(request), "allowAlways")

    def test_deny_from_the_shell(self):
        proc, request = self.ask()
        self.bridge.say(type="decide", id=request["id"], behavior="deny", message="not on battery")
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"], {"behavior": "deny", "message": "not on battery"})
        self.assertEqual(self.resolved(request), "deny")

    def test_allow_from_the_toast(self):
        proc, request = self.ask(command="echo '<b>' && true")
        args = self.bridge.note_args(request["id"])
        self.assertIn("--category=permission", args)
        self.assertIn("--app-name=claude", args)
        self.assertIn("--urgency=normal", args)
        self.assertIn("--action=allow=allow", args)
        self.assertIn("--action=deny=deny", args)
        self.assertIn("--hint=string:x-hecate-claude-request:" + request["id"], args)
        self.assertEqual(args[-3:], ["--", "/home/cakes/src/cakeos wants to run echo …", "echo '&lt;b&gt;' &amp;&amp; true"])
        self.bridge.click(request["id"], "allow")
        self.assertEqual(self.finish(proc), ALLOW)
        self.assertEqual(self.resolved(request), "allow")

    def test_deny_from_the_toast(self):
        proc, request = self.ask()
        self.bridge.note_args(request["id"])
        self.bridge.click(request["id"], "deny")
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"]["behavior"], "deny")

    def test_look_and_closing_the_toast_are_not_answers(self):
        proc, request = self.ask()
        self.bridge.note_args(request["id"])
        self.bridge.click(request["id"], "look")
        self.assertEqual(self.bridge.event("look")["session"], "s1")
        proc2, request2 = self.ask(command="nix build .#b")
        self.bridge.note_args(request2["id"])
        self.bridge.click(request2["id"], "")  # closed without an action
        time.sleep(0.3)
        self.assertIsNone(proc.poll())
        self.assertIsNone(proc2.poll())
        self.bridge.say(type="decide", id=request["id"], behavior="deny")
        self.bridge.say(type="pass", id=request2["id"])
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"]["behavior"], "deny")
        self.assertIsNone(self.finish(proc2))

    def test_unknown_toast_action_is_not_an_answer(self):
        proc, request = self.ask()
        self.bridge.note_args(request["id"])
        self.bridge.click(request["id"], "allow-everything")
        time.sleep(0.3)
        self.assertIsNone(proc.poll())
        self.bridge.say(type="pass", id=request["id"])
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "handedBack")

    def test_nonsense_from_the_shell_is_not_an_answer(self):
        proc, request = self.ask()
        self.bridge.proc.stdin.write(b"allow\n[]\n{}\n\xff\n")
        for message in ({"type": "decide", "id": request["id"]},
                        {"type": "decide", "id": request["id"], "behavior": "ask"},
                        {"type": "decide", "id": request["id"], "behavior": True},
                        {"type": "decide", "id": "f" * 32, "behavior": "allow"},
                        {"type": "allow", "id": request["id"]},
                        {"type": "decide", "behavior": "allow"}):
            self.bridge.say(**message)
        time.sleep(0.4)
        self.assertIsNone(proc.poll())
        self.bridge.say(type="decide", id=request["id"], behavior="deny")
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"]["behavior"], "deny")

    def test_two_requests_get_their_own_answers(self):
        first, a = self.ask(command="nix build .#a")
        second, b = self.ask(command="nix build .#b", session="s2")
        self.bridge.say(type="decide", id=b["id"], behavior="deny")
        self.assertEqual(self.finish(second)["hookSpecificOutput"]["decision"]["behavior"], "deny")
        self.assertIsNone(first.poll())
        self.bridge.say(type="decide", id=a["id"], behavior="allow")
        self.assertEqual(self.finish(first), ALLOW)


class NoAnswer(Case):
    def test_answered_in_the_terminal(self):
        tool = {"tool_name": "Bash", "tool_input": {"command": "nix build .#cakeos"}}
        self.run_hook("PreToolUse", tool_use_id="toolu_1", **tool)
        proc, request = self.ask()
        self.run_hook("PostToolUse", tool_use_id="toolu_1", **tool)
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "terminal")
        self.assertTrue(self.bridge.note_closed(request["id"]))
        self.bridge.click(request["id"], "allow")  # a late click on the withdrawn toast
        self.bridge.say(type="decide", id=request["id"], behavior="allow")
        self.run_hook("SessionStart", session="s2", source="startup")
        state = self.bridge.settle(lambda s: len(s["sessions"]) == 2)
        self.assertEqual(state["requests"], [])
        self.assertEqual([s["state"] for s in state["sessions"] if s["id"] == "s1"], ["working"])
        self.assertEqual(len(self.bridge.events("permissionResolved")), 1)

    def test_answered_in_the_terminal_seen_in_the_transcript(self):
        tool = {"tool_name": "Bash", "tool_input": {"command": "nix build .#cakeos"}}
        transcript = os.path.join(self.runtime, "s1.jsonl")
        with open(transcript, "w") as f:
            f.write(json.dumps({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "toolu_2"}]}}) + "\n")
        self.run_hook("PreToolUse", tool_use_id="toolu_2", **tool)
        proc, request = self.ask()
        time.sleep(1.2)  # the bridge notes where the transcript ends
        self.assertIsNone(proc.poll())
        with open(transcript, "a") as f:
            f.write(json.dumps({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "toolu_2", "is_error": True}]}}) + "\n")
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "terminal")

    def test_refused_in_the_terminal(self):
        # Claude Code fires no hook when the person says no: the transcript is the only word of it
        tool = {"tool_name": "Bash", "tool_input": {"command": "nix build .#cakeos"}}
        transcript = os.path.join(self.runtime, "s1.jsonl")
        open(transcript, "w").close()
        self.run_hook("UserPromptSubmit", prompt="build")
        self.run_hook("PreToolUse", tool_use_id="toolu_3", **tool)
        proc, request = self.ask()
        time.sleep(1.2)
        refusal = "The user doesn't want to proceed with this tool use. The tool use was rejected."
        with open(transcript, "a") as f:
            f.write(json.dumps({"type": "user", "message": {"content": [{"type": "tool_result", "content": refusal, "is_error": True, "tool_use_id": "toolu_3"}]}}) + "\n")
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "terminal")
        state = self.bridge.settle(lambda s: not s["requests"])
        self.assertEqual((state["sessions"][0]["state"], state["sessions"][0]["activity"]), ("idle", ""))

    def test_turn_over(self):
        proc, request = self.ask()
        self.run_hook("Stop", last_assistant_message="Stopped.")
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "terminal")

    def test_hook_gone(self):
        proc, request = self.ask()
        proc.kill()
        self.assertEqual(self.resolved(request), "terminal")
        self.assertTrue(self.bridge.note_closed(request["id"]))
        self.bridge.settle(lambda s: not s["requests"])

    def test_hook_gives_up_waiting(self):
        proc, request = self.ask(env={"HECATE_CLAUDE_WAIT": "0.3"})
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "expired")

    def test_shell_gone(self):
        proc, request = self.ask()
        self.bridge.proc.stdin.close()
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.bridge.proc.wait(timeout=10), 0)
        self.assertFalse(os.path.exists(self.bridge.path))
        self.assertTrue(self.bridge.note_closed(request["id"]))

    def test_bridge_terminated(self):
        proc, _request = self.ask()
        self.bridge.proc.send_signal(signal.SIGTERM)
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.bridge.proc.wait(timeout=10), 0)
        self.assertFalse(os.path.exists(self.bridge.path))

    def test_bridge_killed(self):
        proc, _request = self.ask()
        self.bridge.proc.kill()
        self.assertIsNone(self.finish(proc))
        # the socket file is left behind; the next hook finds nobody home
        self.assertEqual(self.run_hook("PermissionRequest", tool_name="Bash", tool_input={"command": "ls"}), b"")

    def test_questions_go_straight_back(self):
        out = self.run_hook("PermissionRequest", tool_name="AskUserQuestion", tool_input={"questions": []})
        self.assertEqual(out, b"")
        state = self.bridge.settle(lambda s: s["sessions"])
        self.assertEqual((state["sessions"][0]["state"], state["sessions"][0]["waitingFor"]), ("waiting", "input"))
        self.assertEqual(state["requests"], [])


class Socket(Case):
    notify = False

    def connect(self):
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect(self.bridge.path)
        self.addCleanup(sock.close)
        return sock

    def test_permissions(self):
        directory = os.lstat(os.path.dirname(self.bridge.path))
        sock = os.lstat(self.bridge.path)
        self.assertEqual(stat.S_IMODE(directory.st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(sock.st_mode), 0o600)
        self.assertTrue(stat.S_ISSOCK(sock.st_mode))
        self.assertEqual((directory.st_uid, sock.st_uid), (os.getuid(), os.getuid()))

    def test_a_socket_client_cannot_answer(self):
        proc, request = self.ask()
        for line in ({"v": 1, "type": "decision", "id": request["id"], "behavior": "allow"},
                     {"v": 1, "type": "expired", "id": request["id"]},
                     {"type": "decide", "id": request["id"], "behavior": "allow", "scope": "once"},
                     {"v": 1, "type": "hook", "id": request["id"], "reply": False,
                      "input": {"session_id": "s1", "hook_event_name": "PermissionGranted", "decision": "allow"}}):
            rogue = self.connect()
            rogue.sendall(json.dumps(line).encode() + b"\n")
            self.assertEqual(rogue.recv(4096), b"")  # hung up on, and told nothing
        self.assertEqual(self.run_hook("SessionStart", session="s2", source="startup"), b"")
        state = self.bridge.settle(lambda s: len(s["sessions"]) == 2)
        self.assertIsNone(proc.poll())
        self.assertEqual([r["id"] for r in state["requests"]], [request["id"]])
        self.assertEqual(self.bridge.events("permissionResolved"), [])
        self.bridge.say(type="pass", id=request["id"])
        self.assertIsNone(self.finish(proc))
        self.assertEqual(self.resolved(request), "handedBack")

    def test_garbage_and_floods_are_dropped(self):
        for data in (b"\n", b"not json\n", b"[]\n", b'{"v":1}\n', b"\xff\xfe\n"):
            sock = self.connect()
            sock.sendall(data)
            self.assertEqual(sock.recv(4096), b"")
        sock = self.connect()
        try:
            sock.sendall(b"x" * ((1 << 20) + 65536))
            self.assertEqual(sock.recv(4096), b"")
        except OSError:
            pass
        self.assertEqual(self.run_hook("SessionStart", source="startup"), b"")
        self.bridge.settle(lambda s: len(s["sessions"]) == 1)

    def test_silent_client_is_dropped(self):
        sock = self.connect()
        sock.settimeout(15)
        start = time.monotonic()
        self.assertEqual(sock.recv(4096), b"")
        self.assertLess(time.monotonic() - start, 12)

    def test_second_bridge_refuses_and_a_dead_ones_socket_is_taken_over(self):
        second = BridgeProcess(self, self.runtime, notify=False)
        error = second.wait(lambda m: m.get("type") in ("hello", "error"))
        self.assertEqual((error["type"], error["code"]), ("error", "running"))
        self.assertEqual(second.proc.wait(timeout=10), 3)
        self.assertTrue(os.path.exists(self.bridge.path))  # the first one's socket is untouched
        self.bridge.proc.kill()
        self.bridge.proc.wait()
        third = BridgeProcess(self, self.runtime, notify=False)
        self.assertEqual(third.wait(lambda m: m.get("type") in ("hello", "error"))["type"], "hello")
        self.assertEqual(stat.S_IMODE(os.lstat(third.path).st_mode), 0o600)

    def test_someone_elses_directory_is_refused(self):
        with tempfile.TemporaryDirectory(prefix="hcb") as runtime:
            os.symlink("/", os.path.join(runtime, "hecate-claude"))
            other = BridgeProcess(self, runtime, notify=False)
            error = other.wait(lambda m: m.get("type") in ("hello", "error"))
            self.assertEqual((error["type"], error["code"]), ("error", "directory"))
            other.stop()

    def test_open_directory_is_closed_again(self):
        self.bridge.stop()
        os.chmod(os.path.dirname(self.bridge.path), 0o777)
        again = BridgeProcess(self, self.runtime, notify=False)
        self.assertEqual(again.wait(lambda m: m.get("type") in ("hello", "error"))["type"], "hello")
        self.assertEqual(stat.S_IMODE(os.lstat(os.path.dirname(again.path)).st_mode), 0o700)


class Sessions(Case):
    notify = False

    def test_a_session_from_start_to_end(self):
        for event, fields in (
            ("SessionStart", {"source": "startup", "model": "claude-x"}),
            ("UserPromptSubmit", {"prompt": "build it"}),
            ("PreToolUse", {"tool_name": "Edit", "tool_input": {"file_path": "/home/cakes/src/cakeos/flake.nix"}, "tool_use_id": "t1"}),
            ("PostToolUse", {"tool_name": "Edit", "tool_input": {"file_path": "/home/cakes/src/cakeos/flake.nix"}, "tool_use_id": "t1",
                             "tool_response": {"big": "x" * 100000}}),
            ("Notification", {"notification_type": "idle_prompt", "message": "Claude is waiting for your input"}),
            ("Stop", {"last_assistant_message": "Built it."}),
        ):
            self.assertEqual(self.run_hook(event, **fields), b"", event)
        self.assertEqual(self.bridge.event("sessionDone")["session"]["summary"], "Built it.")
        state = self.bridge.settle(lambda s: s["sessions"] and s["sessions"][0]["state"] == "done")
        session = state["sessions"][0]
        self.assertEqual((session["summary"], session["name"], session["model"]), ("Built it.", "cakeos", "claude-x"))
        names = [m["name"] for m in self.bridge.seen if m.get("type") == "event"]
        self.assertEqual(names, ["sessionStarted", "notified", "sessionDone"])
        self.run_hook("SessionEnd", reason="other")
        self.bridge.event("sessionEnded")
        self.bridge.say(type="dismiss", session="s1")
        self.bridge.settle(lambda s: not s["sessions"])

    def test_notifications_can_be_switched_off_while_running(self):
        bridge = self.bridge
        bridge.stop()
        self.bridge = BridgeProcess(self, self.runtime, notify=True)
        self.bridge.wait(lambda m: m.get("type") == "hello")
        proc, request = self.ask()
        self.bridge.note_args(request["id"])
        self.bridge.say(type="config", notifications=False)
        self.assertTrue(self.bridge.note_closed(request["id"]))
        proc2, request2 = self.ask(command="nix build .#b")
        time.sleep(0.3)
        self.assertFalse(os.path.exists(os.path.join(self.bridge.notes, request2["id"] + ".args")))
        self.bridge.say(type="decide", id=request["id"], behavior="allow")
        self.bridge.say(type="decide", id=request2["id"], behavior="allow")
        self.assertEqual(self.finish(proc), ALLOW)
        self.assertEqual(self.finish(proc2), ALLOW)

    def test_a_toast_answered_before_it_shows_is_still_withdrawn(self):
        # notify-send takes SIGINT as "close the notification" only once the notification is
        # up; sooner, it dies and leaves the notification behind
        self.bridge.stop()
        self.bridge = BridgeProcess(self, self.runtime, notify=True, env_extra={"FAKE_NOTIFY_SLOW": "0.5"})
        self.bridge.wait(lambda m: m.get("type") == "hello")
        proc, request = self.ask()
        self.bridge.say(type="decide", id=request["id"], behavior="deny")
        self.assertEqual(self.finish(proc)["hookSpecificOutput"]["decision"]["behavior"], "deny")
        self.assertFalse(os.path.exists(os.path.join(self.bridge.notes, request["id"] + ".closed")))
        self.assertTrue(self.bridge.note_closed(request["id"]))
        self.bridge.click(request["id"], "allow")  # nobody is behind the buttons any more

    def test_missing_notify_send_only_turns_notifications_off(self):
        self.bridge.stop()
        self.bridge = BridgeProcess(self, self.runtime, notify=True, extra=["--notify-send", "/nonexistent/notify-send"])
        self.bridge.wait(lambda m: m.get("type") == "hello")
        proc, request = self.ask()
        self.bridge.say(type="decide", id=request["id"], behavior="allow")
        self.assertEqual(self.finish(proc), ALLOW)


if __name__ == "__main__":
    unittest.main()
