"""sessions.py: how hook events become sessions, states and pending requests."""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

import sessions  # noqa: E402

HOME = "/home/cakes"
CWD = "/home/cakes/src/cakeos"
RULE = {"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "nix build:*"}], "behavior": "allow", "destination": "localSettings"}
DIRS = {"type": "addDirectories", "directories": [CWD], "destination": "session"}
MODE = {"type": "setMode", "mode": "acceptEdits", "destination": "session"}


class Clock:
    def __init__(self):
        self.now = 1_800_000_000.0

    def __call__(self):
        return self.now


class Harness(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.store = sessions.Store(home=HOME, clock=self.clock)
        self.count = 0

    def message(self, event, session="s1", reply=None, client=None, **fields):
        self.count += 1
        hook_input = {"session_id": session, "hook_event_name": event, "cwd": CWD, "permission_mode": "default",
                      "transcript_path": "/home/cakes/.claude/projects/x/%s.jsonl" % session}
        hook_input.update(fields)
        who = {"pid": 500, "claude_pid": 400, "time": int(self.clock.now * 1000),
               "ancestors": [[400, "claude"], [300, "nu"], [200, ".kitty-wrapped"], [1, "systemd"]],
               "input_hash": ""}
        if "tool_name" in hook_input:
            who["input_hash"] = "h-%s-%s" % (hook_input["tool_name"], sorted((hook_input.get("tool_input") or {}).items()))
        who.update(client or {})
        return {"v": 1, "type": "hook", "id": "%032x" % self.count, "reply": event == "PermissionRequest" if reply is None else reply,
                "input": hook_input, "client": who}

    def send(self, event, **kwargs):
        self.clock.now += 1
        message = self.message(event, **kwargs)
        self.assertTrue(sessions.validate(message))
        return message["id"], self.store.handle(message)

    def session(self, sid="s1"):
        return next((s for s in self.store.snapshot()["sessions"] if s["id"] == sid), None)

    def events(self):
        return [(e["name"], e.get("outcome", "")) for e in self.store.drain()]

    def ask(self, command="nix build .#cakeos", **kwargs):
        kwargs.setdefault("tool_name", "Bash")
        kwargs.setdefault("tool_input", {"command": command})
        return self.send("PermissionRequest", **kwargs)


class Validate(unittest.TestCase):
    def test_accepts_a_hook_message(self):
        good = {"v": 1, "type": "hook", "id": "a" * 32, "reply": False, "input": {"session_id": "s", "hook_event_name": "Stop"}}
        self.assertTrue(sessions.validate(good))
        self.assertTrue(sessions.validate(dict(good, client={})))

    def test_rejects_everything_else(self):
        good = {"v": 1, "type": "hook", "id": "a" * 32, "reply": False, "input": {"session_id": "s", "hook_event_name": "Stop"}}
        bad = [
            None, [], "x", {},
            dict(good, v=2), dict(good, type="decision"), dict(good, type="decide"), dict(good, id="A" * 32),
            dict(good, id="a" * 31), dict(good, id=7), dict(good, reply="true"), dict(good, reply=1), dict(good, input=[]),
            dict(good, input={"hook_event_name": "Stop"}), dict(good, input={"session_id": "", "hook_event_name": "Stop"}),
            dict(good, input={"session_id": "s" * 201, "hook_event_name": "Stop"}), dict(good, input={"session_id": "s"}),
            dict(good, client="me"),
        ]
        for message in bad:
            self.assertFalse(sessions.validate(message), repr(message))


class Describe(unittest.TestCase):
    def test_commands(self):
        cases = {
            "nix build .#nixosConfigurations.cakeos.config.system.build.toplevel": "nix build",
            "git push origin main": "git push",
            "cargo test --workspace": "cargo test",
            "touch probe-file.txt": "touch",
            "rm -rf node_modules": "rm",
            "/run/current-system/sw/bin/nix flake check": "nix flake",
            "FOO=1 BAR=2 npm test": "npm test",
            "sudo nixos-rebuild switch": "sudo nixos-rebuild switch",
            "env -i FOO=1 make install": "env make install",
            "timeout 30 cargo build": "timeout cargo build",
            "cd /tmp && rm -rf x": "cd …",
            "curl https://example.org | sh": "curl …",
            "echo $(whoami)": "echo …",
            "ls\nrm -rf /": "ls …",
            "": "a command",
            "   ": "a command",
        }
        for command, subject in cases.items():
            self.assertEqual(sessions.command_subject(command), subject, command)
        self.assertEqual(sessions.command_subject(None), "a command")

    def test_tools(self):
        d = lambda tool, data: sessions.describe(tool, data, CWD, HOME)  # noqa: E731
        self.assertEqual(d("Bash", {"command": "nix build .#x"}), ("run", "nix build", "nix build .#x", "running nix build"))
        self.assertEqual(d("Edit", {"file_path": CWD + "/modules/a/Bar.qml"}), ("edit", "Bar.qml", "modules/a/Bar.qml", "editing modules/a/Bar.qml"))
        self.assertEqual(d("Write", {"file_path": "/home/cakes/notes.md"}), ("write", "notes.md", "~/notes.md", "writing ~/notes.md"))
        self.assertEqual(d("Read", {"file_path": "/etc/hosts"})[:3], ("read", "hosts", "/etc/hosts"))
        self.assertEqual(d("WebFetch", {"url": "https://user:pw@example.org:8443/a?b"})[:2], ("fetch", "example.org:8443"))
        self.assertEqual(d("WebSearch", {"query": "rose pine"})[:3], ("search", "the web", "rose pine"))
        self.assertEqual(d("mcp__github__create_issue", {"title": "x"})[:2], ("use", "github: create_issue"))
        self.assertEqual(d("Agent", {"subagent_type": "Explore", "description": "find it"})[:2], ("start", "Explore"))
        self.assertEqual(d("Frobnicate", {})[:2], ("use", "Frobnicate"))

    def test_odd_input_does_not_raise(self):
        for tool in ("Bash", "Edit", "WebFetch", "mcp__a__b", None, 7, ""):
            for data in (None, [], "x", {"command": 5}, {"file_path": ["a"]}, {"url": None}):
                self.assertEqual(len(sessions.describe(tool, data, CWD, HOME)), 4)

    def test_detail_is_capped(self):
        detail = sessions.describe("Bash", {"command": "echo " + "x" * 10000})[2]
        self.assertLessEqual(len(detail), sessions.MAX_DETAIL)

    def test_always_choice_prefers_a_rule(self):
        self.assertEqual(sessions.always_choice([MODE, DIRS, RULE], HOME), (2, "Bash(nix build:*) in this repo"))
        self.assertEqual(sessions.always_choice([MODE, DIRS], HOME), (-1, ""))  # a directory grant is not a rule
        outside = {"type": "addRules", "rules": [{"toolName": "Read", "ruleContent": "/" + HOME + "/notes/**"}], "behavior": "allow", "destination": "session"}
        self.assertEqual(sessions.always_choice([outside], HOME), (0, "Read(~/notes/**) for this session"))
        outside["rules"][0]["ruleContent"] = "//etc/nixos/**"
        self.assertEqual(sessions.always_choice([outside], HOME), (0, "Read(//etc/nixos/**) for this session"))
        self.assertEqual(sessions.always_choice([MODE], HOME), (-1, ""))
        self.assertEqual(sessions.always_choice(None, HOME), (-1, ""))
        self.assertEqual(sessions.always_choice([{"type": "setMode", "mode": "bypassPermissions"}], HOME), (-1, ""))

    def test_terminal(self):
        find = lambda ancestors: sessions.find_terminal({"ancestors": ancestors})  # noqa: E731
        self.assertEqual(find([[400, "claude"], [300, "nu"], [200, "kitty"]])["pid"], 200)
        self.assertEqual(find([[400, "claude"], [200, ".kitty-wrapped"]])["name"], "kitty")
        self.assertEqual(find([[400, "claude"], [200, ".wezterm-gui-wr"]])["name"], "wezterm-gui")
        self.assertEqual(find([[400, "claude"], [200, "gnome-terminal-"]])["name"], "gnome-terminal")
        self.assertEqual(find([[400, "claude"], [200, "foot"]])["name"], "foot")
        self.assertEqual(find([[400, "claude"], [200, "footman"]])["pid"], 0)
        self.assertEqual(find([[400, "claude"], [200, "tmux: server"]])["pid"], 0)
        self.assertEqual(find("nope")["pid"], 0)
        self.assertEqual(find([["x", "kitty"], [1], None])["pid"], 0)


class Lifecycle(Harness):
    def test_a_turn(self):
        self.send("SessionStart", source="startup", model="claude-x")
        s = self.session()
        self.assertEqual((s["state"], s["dir"], s["name"], s["model"], s["pid"]), ("idle", "~/src/cakeos", "cakeos", "claude-x", 400))
        self.assertEqual(s["terminal"]["name"], "kitty")
        self.assertEqual(s["terminal"]["pid"], 200)

        self.send("UserPromptSubmit", prompt="build it")
        self.assertEqual(self.session()["state"], "working")
        self.assertGreater(self.session()["turnStartedAt"], 0)

        self.send("PreToolUse", tool_name="Edit", tool_input={"file_path": CWD + "/flake.nix"}, tool_use_id="toolu_1")
        s = self.session()
        self.assertEqual((s["state"], s["activity"], s["tool"]), ("working", "editing flake.nix", "Edit"))

        self.send("PostToolUse", tool_name="Edit", tool_input={"file_path": CWD + "/flake.nix"}, tool_use_id="toolu_1")
        self.send("Stop", last_assistant_message="Done.\n\nTwo files   changed.", stop_hook_active=False)
        s = self.session()
        self.assertEqual((s["state"], s["summary"], s["activity"], s["turnStartedAt"], s["error"]), ("done", "Done. Two files changed.", "", 0, ""))
        self.assertGreater(s["doneAt"], 0)
        self.assertEqual(self.events(), [("sessionStarted", ""), ("sessionDone", "")])

        self.send("SessionEnd", reason="other")
        s = self.session()
        self.assertTrue(s["ended"])  # kept: it has a result to show
        self.assertEqual(self.events(), [("sessionEnded", "")])
        self.assertTrue(self.store.dismiss("s1"))
        self.assertIsNone(self.session())

    def test_session_without_a_result_goes_when_it_ends(self):
        self.send("SessionStart", source="startup")
        self.send("SessionEnd", reason="prompt_input_exit")
        self.assertIsNone(self.session())

    def test_failed_turn(self):
        self.send("UserPromptSubmit")
        self.send("StopFailure", error="rate_limit", last_assistant_message="API Error: Rate limit reached")
        s = self.session()
        self.assertEqual((s["state"], s["error"], s["summary"]), ("done", "rate_limit", "API Error: Rate limit reached"))

    def test_summary_is_capped(self):
        self.send("Stop", last_assistant_message="word " * 1000)
        self.assertLessEqual(len(self.session()["summary"]), sessions.MAX_SUMMARY)

    def test_sessions_are_listed_waiting_first(self):
        self.send("Stop", session="done", last_assistant_message="ok")
        self.send("UserPromptSubmit", session="working")
        self.send("SessionStart", session="idle")
        self.ask(session="waiting")
        self.assertEqual([s["id"] for s in self.store.snapshot()["sessions"]], ["waiting", "working", "idle", "done"])

    def test_notifications(self):
        self.send("UserPromptSubmit")
        self.send("Notification", notification_type="elicitation_dialog", message="A server asks")
        s = self.session()
        self.assertEqual((s["state"], s["waitingFor"], s["message"]), ("waiting", "input", "A server asks"))
        self.send("PostToolUse", tool_name="mcp__a__b", tool_input={}, tool_use_id="toolu_9")
        self.assertEqual(self.session()["state"], "working")
        self.send("Stop", last_assistant_message="ok")
        self.send("Notification", notification_type="idle_prompt", message="Claude is waiting for your input")
        self.assertEqual(self.session()["state"], "done")
        names = [name for name, _ in self.events()]
        self.assertEqual(names.count("notified"), 2)

    def test_subagents(self):
        self.send("SubagentStart", agent_id="a1", agent_type="Explore")
        self.send("SubagentStart", agent_id="a2", agent_type="Explore")
        self.send("SubagentStop", agent_id="a1", agent_type="Explore")
        self.assertEqual(self.session()["subagents"], 1)
        self.send("SubagentStop", agent_id="a2")
        self.send("SubagentStop", agent_id="a3")
        self.assertEqual(self.session()["subagents"], 0)

    def test_late_tool_event_does_not_wake_a_done_session(self):
        self.send("UserPromptSubmit")
        before = int(self.clock.now * 1000)
        self.send("Stop", last_assistant_message="ok")
        self.send("PostToolUse", tool_name="Bash", tool_input={"command": "ls"}, tool_use_id="t", client={"time": before})
        self.send("PreToolUse", tool_name="Bash", tool_input={"command": "ls"}, tool_use_id="t", client={"time": before})
        self.assertEqual(self.session()["state"], "done")

    def test_unknown_event_only_touches_the_session(self):
        self.send("UserPromptSubmit")
        self.send("SomethingNew", whatever=[1, 2, 3])
        self.assertEqual(self.session()["state"], "working")

    def test_dead_sessions_are_forgotten(self):
        self.send("UserPromptSubmit", session="alive")
        self.send("UserPromptSubmit", session="dead", client={"claude_pid": 999})
        self.send("Stop", session="result", last_assistant_message="ok", client={"claude_pid": 998})
        rid, _ = self.ask(session="dead", client={"claude_pid": 999})
        self.store.drain()
        self.store.tick(lambda pid: pid == 400)
        self.assertIsNotNone(self.session("alive"))
        self.assertIsNone(self.session("dead"))
        self.assertTrue(self.session("result")["ended"])
        self.assertNotIn(rid, self.store.requests)
        self.assertIn(("permissionResolved", "terminal"), self.events())
        self.clock.now += sessions.ENDED_TTL + 1
        self.store.tick(lambda pid: pid == 400)
        self.assertIsNone(self.session("result"))

    def test_session_limit(self):
        for i in range(sessions.MAX_SESSIONS + 5):
            self.send("SessionStart", session="s%d" % i)
        self.assertEqual(len(self.store.sessions), sessions.MAX_SESSIONS)
        _, result = self.ask(session="one-too-many")
        self.assertEqual(result[0], "pass")


class Requests(Harness):
    def test_request_is_held_and_described(self):
        rid, result = self.ask(permission_suggestions=[MODE, RULE])
        self.assertEqual(result[0], "hold")
        request = result[1]
        self.assertEqual(request["id"], rid)
        self.assertEqual(request["title"], "~/src/cakeos wants to run nix build")
        self.assertEqual((request["tool"], request["verb"], request["subject"], request["detail"]), ("Bash", "run", "nix build", "nix build .#cakeos"))
        self.assertEqual((request["canAlways"], request["alwaysLabel"]), (True, "Bash(nix build:*) in this repo"))
        s = self.session()
        self.assertEqual((s["state"], s["waitingFor"], s["requests"]), ("waiting", "permission", [rid]))
        self.assertEqual(self.store.snapshot()["requests"][0]["id"], rid)
        self.assertIn(("permissionRequested", ""), self.events())

    def test_a_long_directory_is_cut_in_the_title_only(self):
        self.assertEqual(sessions.brief("~/src/cakeos"), "~/src/cakeos")
        self.assertEqual(sessions.brief("/tmp/a-very-long-scratch-directory/probe/work"), "\u2026/probe/work")
        self.assertEqual(sessions.brief("~/src/an-unreasonably-long-project-name/sub"), "\u2026/sub")
        self.assertEqual(sessions.brief("/" + "x" * 80), "\u2026/" + "x" * 26)
        self.assertEqual(sessions.brief(""), "")

    def test_allow_once(self):
        rid, _ = self.ask(permission_suggestions=[RULE])
        self.assertEqual(self.store.decide(rid, "allow"), {"v": 1, "type": "decision", "id": rid, "behavior": "allow"})
        self.assertEqual(self.session()["state"], "working")
        self.assertEqual(self.store.snapshot()["requests"], [])
        self.assertIn(("permissionResolved", "allow"), self.events())

    def test_allow_always_points_at_the_suggestion(self):
        rid, _ = self.ask(permission_suggestions=[MODE, RULE])
        self.assertEqual(self.store.decide(rid, "allow", "always"), {"v": 1, "type": "decision", "id": rid, "behavior": "allow", "suggestion": 1})
        self.assertIn(("permissionResolved", "allowAlways"), self.events())

    def test_allow_always_without_a_suggestion_allows_once(self):
        rid, _ = self.ask(permission_suggestions=[MODE])
        self.assertEqual(self.store.decide(rid, "allow", "always"), {"v": 1, "type": "decision", "id": rid, "behavior": "allow"})
        self.assertIn(("permissionResolved", "allow"), self.events())

    def test_deny(self):
        rid, _ = self.ask()
        self.assertEqual(self.store.decide(rid, "deny")["message"], "Denied from the desktop.")
        rid, _ = self.ask()
        wire = self.store.decide(rid, "deny", message="use the flake", interrupt=True)
        self.assertEqual((wire["behavior"], wire["message"], wire["interrupt"]), ("deny", "use the flake", True))

    def test_only_allow_and_deny_are_answers(self):
        rid, _ = self.ask()
        for behavior in ("ask", "", None, "Allow", True, "allow ", "approve"):
            self.assertIsNone(self.store.decide(rid, behavior), repr(behavior))
        self.assertIn(rid, self.store.requests)

    def test_a_request_is_answered_once(self):
        rid, _ = self.ask()
        self.assertIsNotNone(self.store.decide(rid, "deny"))
        self.assertIsNone(self.store.decide(rid, "allow"))
        self.assertIsNone(self.store.decide("f" * 32, "allow"))

    def test_an_answer_for_one_request_leaves_the_others(self):
        first, _ = self.ask("nix build .#a")
        second, _ = self.ask("nix build .#b")
        self.store.decide(first, "allow")
        self.assertEqual(self.session()["state"], "waiting")
        self.assertEqual([r["id"] for r in self.store.snapshot()["requests"]], [second])

    def test_event_without_reply_is_not_held(self):
        _, result = self.ask(reply=False)
        self.assertIsNone(result)
        self.assertEqual(self.store.requests, {})
        self.assertEqual(self.session()["state"], "waiting")

    def test_other_events_are_never_held(self):
        for event in ("PreToolUse", "PostToolUse", "Stop", "Notification", "SessionStart", "SessionEnd", "UserPromptSubmit"):
            _, result = self.send(event, reply=True, tool_name="Bash", tool_input={"command": "ls"})
            self.assertIsNone(result, event)
        self.assertEqual(self.store.requests, {})

    def test_questions_and_plans_go_to_the_terminal(self):
        for tool in sessions.TERMINAL_ONLY_TOOLS:
            _, result = self.ask(tool_name=tool, tool_input={})
            self.assertEqual(result, ("pass", "needs the terminal"))
            s = self.session()
            self.assertEqual((s["state"], s["waitingFor"]), ("waiting", "input"))
        self.assertEqual(self.store.requests, {})

    def test_answered_in_the_terminal_seen_by_tool_result(self):
        tool = {"tool_name": "Bash", "tool_input": {"command": "nix build .#cakeos"}}
        self.send("PreToolUse", tool_use_id="toolu_7", **tool)
        rid, _ = self.ask()
        self.assertEqual(self.store.watch_list(), [(rid, "/home/cakes/.claude/projects/x/s1.jsonl", "toolu_7")])
        self.send("PostToolUse", tool_use_id="toolu_7", **tool)
        self.assertEqual(self.store.requests, {})
        self.assertEqual(self.session()["state"], "working")
        self.assertIn(("permissionResolved", "terminal"), self.events())

    def test_pre_tool_use_arriving_late_still_links_the_call(self):
        rid, _ = self.ask()
        self.assertEqual(self.store.watch_list(), [])
        self.send("PreToolUse", tool_use_id="toolu_8", tool_name="Bash", tool_input={"command": "nix build .#cakeos"})
        self.assertEqual(self.store.watch_list()[0][2], "toolu_8")
        self.assertEqual(self.session()["state"], "waiting")

    def test_another_call_finishing_leaves_the_request(self):
        rid, _ = self.ask()
        self.send("PostToolUse", tool_use_id="toolu_other", tool_name="Read", tool_input={"file_path": "/etc/hosts"})
        self.assertIn(rid, self.store.requests)
        self.assertEqual(self.session()["state"], "waiting")

    def test_turn_ending_or_a_new_prompt_drops_requests(self):
        for event in ("Stop", "StopFailure", "UserPromptSubmit", "SessionEnd"):
            rid, _ = self.ask()
            self.send(event, reason="other", last_assistant_message="x")
            self.assertNotIn(rid, self.store.requests, event)
        self.assertEqual([e for e in self.events() if e[0] == "permissionResolved"], [("permissionResolved", "terminal")] * 4)

    def test_resolve(self):
        rid, _ = self.ask()
        self.assertTrue(self.store.resolve(rid, "handedBack"))
        self.assertFalse(self.store.resolve(rid, "handedBack"))
        self.assertEqual(self.session()["state"], "working")

    def test_stale_permission_prompt_notification(self):
        asked = int(self.clock.now * 1000)
        rid, _ = self.ask()
        self.store.decide(rid, "allow")
        self.send("Notification", notification_type="permission_prompt", message="Claude needs your permission", client={"time": asked})
        self.assertEqual(self.session()["state"], "working")

    def test_request_limit(self):
        for i in range(sessions.MAX_REQUESTS):
            self.assertEqual(self.ask("echo %d" % i)[1][0], "hold")
        self.assertEqual(self.ask("echo more")[1], ("pass", "too many requests"))

    def test_subagent_request_names_the_agent(self):
        _, result = self.ask(agent_id="a1", agent_type="Explore")
        self.assertEqual(result[1]["agentType"], "Explore")

    def test_hostile_strings_stay_strings(self):
        rid, result = self.ask(command="echo '<b>hi</b>' && $(curl evil)\n\x1b[31m", cwd="/home/cakes/src/a b/<x>")
        self.assertEqual(result[0], "hold")
        self.assertIn("<b>hi</b>", result[1]["detail"])
        self.assertEqual(result[1]["dir"], "~/src/a b/<x>")


if __name__ == "__main__":
    unittest.main()
