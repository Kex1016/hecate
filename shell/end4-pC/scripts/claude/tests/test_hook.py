"""hook.py: what it prints, and above all when it prints nothing."""

import io
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import hook  # noqa: E402
import sessions  # noqa: E402

HOOK = os.path.join(os.path.dirname(HERE), "hook.py")
RID = "0123456789abcdef0123456789abcdef"

RULE = {"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "nix build:*"}], "behavior": "allow", "destination": "localSettings"}
DIRS = {"type": "addDirectories", "directories": ["/home/cakes/src/cakeos"], "destination": "session"}
MODE = {"type": "setMode", "mode": "acceptEdits", "destination": "session"}
BYPASS = {"type": "setMode", "mode": "bypassPermissions", "destination": "session"}
DENY_RULE = {"type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "deny", "destination": "session"}


def permission_input(**extra):
    data = {
        "session_id": "sess-1",
        "hook_event_name": "PermissionRequest",
        "cwd": "/home/cakes/src/cakeos",
        "permission_mode": "default",
        "tool_name": "Bash",
        "tool_input": {"command": "nix build .#cakeos"},
        "permission_suggestions": [MODE, RULE, DIRS, BYPASS, DENY_RULE],
    }
    data.update(extra)
    return data


def decision(**fields):
    message = {"v": 1, "type": "decision", "id": RID, "behavior": "allow"}
    message.update(fields)
    return message


class BuildOutput(unittest.TestCase):
    def test_allow(self):
        out = hook.build_output(permission_input(), RID, decision())
        self.assertEqual(out, {"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})

    def test_deny_with_message(self):
        out = hook.build_output(permission_input(), RID, decision(behavior="deny", message="  not now  "))
        self.assertEqual(out["hookSpecificOutput"]["decision"], {"behavior": "deny", "message": "not now"})

    def test_deny_can_stop_claude(self):
        out = hook.build_output(permission_input(), RID, decision(behavior="deny", interrupt=True))
        self.assertEqual(out["hookSpecificOutput"]["decision"], {"behavior": "deny", "interrupt": True})

    def test_deny_message_is_capped(self):
        out = hook.build_output(permission_input(), RID, decision(behavior="deny", message="x" * 5000))
        self.assertEqual(len(out["hookSpecificOutput"]["decision"]["message"]), hook.MAX_MESSAGE)

    def test_always_echoes_claude_codes_own_suggestion(self):
        out = hook.build_output(permission_input(), RID, decision(suggestion=1))
        self.assertEqual(out["hookSpecificOutput"]["decision"], {"behavior": "allow", "updatedPermissions": [RULE]})

    def test_always_never_changes_the_mode_grants_a_directory_or_adds_a_deny_rule(self):
        for index in (0, 2, 3, 4):
            self.assertIsNone(hook.build_output(permission_input(), RID, decision(suggestion=index)), index)

    def test_bad_suggestion_is_no_decision(self):
        for bad in (-1, 5, 99, "1", 1.0, True, None, [1], {"type": "addRules"}):
            self.assertIsNone(hook.build_output(permission_input(), RID, decision(suggestion=bad)), repr(bad))
        self.assertIsNone(hook.build_output(permission_input(permission_suggestions=None), RID, decision(suggestion=0)))

    def test_a_rule_sent_over_the_socket_is_ignored(self):
        # The bridge can only point at a suggestion; it cannot hand one over.
        out = hook.build_output(permission_input(), RID, decision(updatedPermissions=[BYPASS], updatedInput={"command": "rm -rf /"}))
        self.assertEqual(out["hookSpecificOutput"]["decision"], {"behavior": "allow"})

    def test_only_permission_request_is_answered(self):
        for event in ("PreToolUse", "PostToolUse", "Stop", "Notification", "SessionStart", "UserPromptSubmit", "permissionrequest", ""):
            self.assertIsNone(hook.build_output(permission_input(hook_event_name=event), RID, decision()), event)

    def test_anything_else_is_no_decision(self):
        good = permission_input()
        bad_messages = [
            None, "allow", 1, [], {},
            decision(v=2), decision(v="1"), decision(v=None),
            decision(type="pass"), decision(type="pending"), decision(type="allow"), decision(type=None),
            decision(id="f" * 32), decision(id=""), decision(id=None),
            decision(behavior="ask"), decision(behavior="Allow"), decision(behavior="allow "), decision(behavior=True),
            decision(behavior=None), decision(behavior=["allow"]), decision(behavior="approve"), decision(behavior="yes"),
        ]
        for message in bad_messages:
            self.assertIsNone(hook.build_output(good, RID, message), repr(message))
        no_behavior = decision()
        del no_behavior["behavior"]
        self.assertIsNone(hook.build_output(good, RID, no_behavior))
        self.assertIsNone(hook.build_output(good, "", decision(id="")))
        self.assertIsNone(hook.build_output(None, RID, decision()))

    def test_hook_and_bridge_agree_on_what_always_may_apply(self):
        entries = [RULE, DIRS, MODE, BYPASS, DENY_RULE, {}, None, {"type": "addRules", "behavior": "allow", "rules": []},
                   {"type": "addRules", "behavior": "allow", "rules": [{"ruleContent": "x"}]},
                   {"type": "addDirectories", "directories": [1]}, {"type": "addDirectories", "directories": []}]
        for index, entry in enumerate(entries):
            ours = hook.always_entry({"permission_suggestions": entries}, index) is not None
            self.assertEqual(ours, sessions.always_entry_ok(entry), repr(entry))


class Message(unittest.TestCase):
    def test_shape(self):
        message = hook.build_message(permission_input(), RID, {"CLAUDE_PID": str(os.getpid())})
        self.assertTrue(sessions.validate(message))
        self.assertTrue(message["reply"])
        self.assertEqual(message["client"]["claude_pid"], os.getpid())
        self.assertEqual(message["client"]["ancestors"][0][0], os.getpid())
        self.assertEqual(len(message["client"]["input_hash"]), 32)

    def stand_in(self, *args):
        """A process with Claude Code's arguments."""
        proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"] + list(args))
        self.addCleanup(proc.wait)
        self.addCleanup(proc.kill)
        return proc.pid

    def test_headless_is_read_from_claude_codes_command_line(self):
        def headless(*args):
            return hook.build_message(permission_input(), RID, {"CLAUDE_PID": str(self.stand_in(*args))})["client"]["headless"]

        self.assertTrue(headless("-p", "say hi"))
        self.assertTrue(headless("--model", "x", "--print"))
        self.assertFalse(headless("--model", "x"))
        self.assertFalse(headless("--permission-mode", "default", "explain -p to me"))
        self.assertFalse(hook.is_headless(0x3FFFFFFF))  # nobody: then it is not assumed
        # Claude Code's own word wins over the command line
        printing, interactive = self.stand_in("-p", "x"), self.stand_in()
        self.assertFalse(hook.is_headless(printing, {"CLAUDE_CODE_SESSION_ATTENDED": "1"}))
        self.assertTrue(hook.is_headless(interactive, {"CLAUDE_CODE_SESSION_ATTENDED": "0"}))
        self.assertTrue(hook.is_headless(printing, {"CLAUDE_CODE_SESSION_ATTENDED": "maybe"}))

    def test_only_permission_request_waits_for_a_reply(self):
        for event in ("PreToolUse", "Stop", "Notification", "SessionEnd"):
            self.assertFalse(hook.build_message(permission_input(hook_event_name=event), RID, {})["reply"])

    def test_big_inputs_are_cut_and_tool_output_dropped(self):
        big = permission_input(hook_event_name="PostToolUse", tool_input={"content": "x" * 3_000_000}, tool_response={"stdout": "y" * 3_000_000})
        message = hook.build_message(big, RID, {})
        self.assertNotIn("tool_response", message["input"])
        self.assertLess(len(json.dumps(message)), 20000)

    def test_one_call_has_one_hash(self):
        a = hook.input_hash(permission_input(hook_event_name="PreToolUse", tool_use_id="toolu_1"))
        b = hook.input_hash(permission_input())
        c = hook.input_hash(permission_input(tool_input={"command": "nix build .#other"}))
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertEqual(hook.input_hash({"hook_event_name": "Stop"}), "")


class SocketChecks(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = os.path.join(self.tmp.name, "hecate-claude")
        os.mkdir(self.dir, 0o700)
        self.path = os.path.join(self.dir, "bridge.sock")

    def tearDown(self):
        self.tmp.cleanup()

    def listen(self):
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(self.path)
        server.listen(4)
        self.addCleanup(server.close)
        return server

    def test_default_path(self):
        self.assertEqual(hook.socket_path({"XDG_RUNTIME_DIR": "/run/user/1000"}), "/run/user/1000/hecate-claude/bridge.sock")
        self.assertEqual(hook.socket_path({}, uid=1000), "/run/user/1000/hecate-claude/bridge.sock")
        self.assertEqual(hook.socket_path({"HECATE_CLAUDE_SOCKET": "/x/y.sock", "XDG_RUNTIME_DIR": "/run/user/1000"}), "/x/y.sock")

    def test_missing_socket(self):
        self.assertFalse(hook.path_is_ours(self.path))
        self.assertIsNone(hook.connect(self.path))

    def test_our_socket(self):
        self.listen()
        self.assertTrue(hook.path_is_ours(self.path))
        sock = hook.connect(self.path)
        self.assertIsNotNone(sock)
        sock.close()

    def test_someone_elses_socket_or_directory(self):
        self.listen()
        self.assertFalse(hook.path_is_ours(self.path, uid=os.getuid() + 1))

    def test_directory_others_can_write_to(self):
        self.listen()
        for mode in (0o777, 0o770, 0o702, 0o720):
            os.chmod(self.dir, mode)
            self.assertFalse(hook.path_is_ours(self.path), oct(mode))
            self.assertIsNone(hook.connect(self.path), oct(mode))
        os.chmod(self.dir, 0o700)

    def test_a_file_is_not_a_socket(self):
        open(self.path, "w").close()
        self.assertFalse(hook.path_is_ours(self.path))

    def test_stale_socket_file(self):
        self.listen().close()
        self.assertIsNone(hook.connect(self.path))


class WaitForDecision(unittest.TestCase):
    def pair(self):
        ours, theirs = socket.socketpair()
        self.addCleanup(ours.close)
        self.addCleanup(theirs.close)
        return ours, theirs

    def lines(self, *messages):
        return b"".join(json.dumps(m).encode() + b"\n" for m in messages)

    def wait(self, ours, **kwargs):
        kwargs.setdefault("out_fd", -1)
        kwargs.setdefault("pending_timeout", 0.3)
        return hook.wait_for_decision(ours, RID, **kwargs)

    def test_pending_then_decision(self):
        ours, theirs = self.pair()
        theirs.sendall(self.lines({"v": 1, "type": "pending", "id": RID}, decision()))
        self.assertEqual(self.wait(ours), decision())

    def test_decision_split_across_packets(self):
        ours, theirs = self.pair()
        data = self.lines({"v": 1, "type": "pending", "id": RID}, decision(behavior="deny"))

        def drip():
            for i in range(0, len(data), 7):
                theirs.sendall(data[i : i + 7])
                time.sleep(0.002)

        threading.Thread(target=drip, daemon=True).start()
        self.assertEqual(self.wait(ours, pending_timeout=2.0)["behavior"], "deny")

    def test_no_pending_in_time(self):
        ours, _theirs = self.pair()
        start = time.monotonic()
        self.assertIsNone(self.wait(ours))
        self.assertLess(time.monotonic() - start, 2.0)

    def test_decision_without_pending(self):
        ours, theirs = self.pair()
        theirs.sendall(self.lines(decision()))
        self.assertIsNone(self.wait(ours))

    def test_pass_closed_garbage_and_unknown(self):
        pending = {"v": 1, "type": "pending", "id": RID}
        cases = [
            self.lines(pending, {"v": 1, "type": "pass", "id": RID}),
            self.lines(pending) + b"allow\n",
            self.lines(pending) + b"\xff\xfe\n",
            self.lines(pending) + b"[1]\n",
            self.lines(pending, {"v": 1, "type": "surprise", "id": RID}),
            self.lines(pending, decision(id="f" * 32)),
            self.lines(pending, decision(v=2)),
            self.lines({"v": 1, "type": "pending", "id": "f" * 32}, decision()),
            b"",
        ]
        for data in cases:
            ours, theirs = self.pair()
            theirs.sendall(data)
            theirs.close()
            self.assertIsNone(self.wait(ours), data)

    def test_oversized_line(self):
        ours, theirs = self.pair()
        theirs.sendall(self.lines({"v": 1, "type": "pending", "id": RID}))

        def flood():
            try:
                theirs.sendall(b"x" * (hook.MAX_LINE + 4096))
            except OSError:
                pass

        threading.Thread(target=flood, daemon=True).start()
        self.assertIsNone(self.wait(ours))

    def test_max_wait(self):
        ours, theirs = self.pair()
        theirs.sendall(self.lines({"v": 1, "type": "pending", "id": RID}))
        start = time.monotonic()
        self.assertIsNone(self.wait(ours, max_wait=0.3))
        self.assertLess(time.monotonic() - start, 2.0)

    def test_claude_code_stopped_listening(self):
        ours, theirs = self.pair()
        theirs.sendall(self.lines({"v": 1, "type": "pending", "id": RID}))
        read_end, write_end = os.pipe()
        os.close(read_end)  # nobody reads our stdout any more
        self.addCleanup(os.close, write_end)
        start = time.monotonic()
        self.assertIsNone(self.wait(ours, out_fd=write_end))
        self.assertLess(time.monotonic() - start, 2.0)


class FakeBridge:
    """A socket that answers each connection with the given bytes."""

    def __init__(self, test, reply=b"", close=True):
        self.tmp = tempfile.TemporaryDirectory()
        test.addCleanup(self.tmp.cleanup)
        os.mkdir(os.path.join(self.tmp.name, "hecate-claude"), 0o700)
        self.path = os.path.join(self.tmp.name, "hecate-claude", "bridge.sock")
        self.server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.server.bind(self.path)
        self.server.listen(8)
        test.addCleanup(self.server.close)
        self.reply = reply
        self.close = close
        self.received = []
        self.kept = []
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                conn, _ = self.server.accept()
            except OSError:
                return
            data = b""
            while b"\n" not in data:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                data += chunk
            message = json.loads(data.decode() or "null")
            self.received.append(message)
            reply = self.reply(message) if callable(self.reply) else self.reply
            try:
                conn.sendall(reply)
            except OSError:
                pass
            if self.close:
                conn.close()
            else:
                self.kept.append(conn)


def run_hook(hook_input, runtime_dir, extra_env=None, timeout=20, raw=None):
    env = {"PATH": os.environ.get("PATH", ""), "XDG_RUNTIME_DIR": runtime_dir}
    env.update(extra_env or {})
    data = raw if raw is not None else json.dumps(hook_input).encode()
    return subprocess.run([sys.executable, HOOK], input=data, capture_output=True, env=env, timeout=timeout)


class EndToEnd(unittest.TestCase):
    def answer(self, **fields):
        def reply(message):
            lines = [{"v": 1, "type": "pending", "id": message["id"]}, dict({"v": 1, "type": "decision", "id": message["id"]}, **fields)]
            return b"".join(json.dumps(m).encode() + b"\n" for m in lines)

        return reply

    def test_no_bridge(self):
        with tempfile.TemporaryDirectory() as tmp:
            done = run_hook(permission_input(), tmp)
        self.assertEqual((done.returncode, done.stdout, done.stderr), (0, b"", b""))

    def test_no_runtime_directory(self):
        done = run_hook(permission_input(), "/nonexistent/hecate-test")
        self.assertEqual((done.returncode, done.stdout), (0, b""))

    def test_allow(self):
        bridge = FakeBridge(self, self.answer(behavior="allow"))
        done = run_hook(permission_input(), bridge.tmp.name)
        self.assertEqual(done.returncode, 0)
        self.assertEqual(json.loads(done.stdout), {"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})
        self.assertEqual(done.stdout.count(b"\n"), 1)
        self.assertTrue(sessions.validate(bridge.received[0]))
        self.assertEqual(bridge.received[0]["input"]["tool_input"], {"command": "nix build .#cakeos"})

    def test_deny(self):
        bridge = FakeBridge(self, self.answer(behavior="deny", message="Denied from the desktop."))
        done = run_hook(permission_input(), bridge.tmp.name)
        self.assertEqual(json.loads(done.stdout)["hookSpecificOutput"]["decision"], {"behavior": "deny", "message": "Denied from the desktop."})

    def test_always(self):
        bridge = FakeBridge(self, self.answer(behavior="allow", suggestion=1))
        done = run_hook(permission_input(), bridge.tmp.name)
        self.assertEqual(json.loads(done.stdout)["hookSpecificOutput"]["decision"], {"behavior": "allow", "updatedPermissions": [RULE]})

    def test_other_events_never_print_even_when_told_to_allow(self):
        bridge = FakeBridge(self, self.answer(behavior="allow"))
        for event in ("PreToolUse", "PostToolUse", "Stop", "Notification", "SessionStart", "SessionEnd", "UserPromptSubmit"):
            done = run_hook(permission_input(hook_event_name=event), bridge.tmp.name)
            self.assertEqual((done.returncode, done.stdout), (0, b""), event)
        self.assertEqual(len(bridge.received), 7)
        self.assertFalse(any(m["reply"] for m in bridge.received))

    def test_broken_answers_print_nothing(self):
        replies = [
            b"",
            b"\n",
            b"allow\n",
            b'{"behavior":"allow"}\n',
            b'{"v":1,"type":"decision","id":"' + b"f" * 32 + b'","behavior":"allow"}\n',
            self.answer(behavior="ask"),
            self.answer(behavior="allow", suggestion=0),
            self.answer(behavior="allow", suggestion=42),
            lambda m: json.dumps({"v": 1, "type": "decision", "id": m["id"], "behavior": "allow"}).encode() + b"\n",
            lambda m: json.dumps({"v": 1, "type": "pending", "id": m["id"]}).encode() + b"\n" + json.dumps({"v": 1, "type": "pass", "id": m["id"]}).encode() + b"\n",
            lambda m: json.dumps({"v": 1, "type": "pending", "id": m["id"]}).encode() + b"\n",
        ]
        for reply in replies:
            bridge = FakeBridge(self, reply)
            done = run_hook(permission_input(), bridge.tmp.name)
            self.assertEqual((done.returncode, done.stdout), (0, b""), repr(reply))

    def test_bridge_that_never_takes_the_request(self):
        bridge = FakeBridge(self, b"", close=False)
        start = time.monotonic()
        done = run_hook(permission_input(), bridge.tmp.name)
        self.assertEqual((done.returncode, done.stdout), (0, b""))
        self.assertLess(time.monotonic() - start, hook.PENDING_TIMEOUT + 5)

    def test_wait_limit(self):
        bridge = FakeBridge(self, lambda m: json.dumps({"v": 1, "type": "pending", "id": m["id"]}).encode() + b"\n", close=False)
        done = run_hook(permission_input(), bridge.tmp.name, {"HECATE_CLAUDE_WAIT": "0.4"})
        self.assertEqual((done.returncode, done.stdout), (0, b""))

    def test_headless_session_waits_only_so_long(self):
        bridge = FakeBridge(self, lambda m: json.dumps({"v": 1, "type": "pending", "id": m["id"]}).encode() + b"\n", close=False)
        printing = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)", "-p", "x"])
        self.addCleanup(printing.wait)
        self.addCleanup(printing.kill)
        environ = {"XDG_RUNTIME_DIR": bridge.tmp.name, "CLAUDE_PID": str(printing.pid)}
        start = time.monotonic()
        with mock.patch.object(hook, "HEADLESS_WAIT", 0.3):
            text = hook.run(io.BytesIO(json.dumps(permission_input()).encode()), io.StringIO(), environ)
        self.assertEqual(text, "")
        self.assertLess(time.monotonic() - start, 5)
        self.assertTrue(bridge.received[0]["client"]["headless"])

    def test_switched_off(self):
        bridge = FakeBridge(self, self.answer(behavior="allow"))
        for value in ("0", "off"):
            done = run_hook(permission_input(), bridge.tmp.name, {"HECATE_CLAUDE_BRIDGE": value})
            self.assertEqual((done.returncode, done.stdout), (0, b""))
        self.assertEqual(bridge.received, [])
        done = run_hook(permission_input(), bridge.tmp.name, {"HECATE_CLAUDE_BRIDGE": "1"})
        self.assertIn(b'"behavior":"allow"', done.stdout)

    def test_input_that_is_not_a_hook_event(self):
        bridge = FakeBridge(self, self.answer(behavior="allow"))
        for raw in (b"", b"not json", b"[]", b'"PermissionRequest"', b'{"session_id":"s"}', b"\xff\xfe"):
            done = run_hook(None, bridge.tmp.name, raw=raw)
            self.assertEqual((done.returncode, done.stdout), (0, b""), raw)
        self.assertEqual(bridge.received, [])

    def test_directory_others_can_write_to(self):
        bridge = FakeBridge(self, self.answer(behavior="allow"))
        os.chmod(os.path.dirname(bridge.path), 0o777)
        done = run_hook(permission_input(), bridge.tmp.name)
        self.assertEqual((done.returncode, done.stdout), (0, b""))
        self.assertEqual(bridge.received, [])


if __name__ == "__main__":
    unittest.main()
