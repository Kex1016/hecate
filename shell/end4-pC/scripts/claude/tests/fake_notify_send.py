#!/usr/bin/env python3
"""A stand-in for notify-send in the bridge tests, with the timing of the real one.

Like notify-send 0.8: with --print-id it prints the notification's id once the notification
is showing, and only after that does SIGINT withdraw the notification (earlier, SIGINT just
kills it and a notification that was already sent would stay up). An action is printed when
the "user" clicks it, then it exits.

It writes its arguments to $FAKE_NOTIFY_DIR/<request id>.args when the notification "shows"
(after $FAKE_NOTIFY_SLOW seconds), waits for <request id>.action and prints its content; an
empty action file means the toast was closed without an answer. On SIGINT it writes
<request id>.closed.
"""
import json
import os
import signal
import sys
import time

directory = os.environ["FAKE_NOTIFY_DIR"]
request = "unknown"
for arg in sys.argv[1:]:
    if arg.startswith("--hint=string:x-hecate-claude-request:"):
        request = arg.rsplit(":", 1)[1]
base = os.path.join(directory, request)


def closed(_signum, _frame):
    open(base + ".closed", "w").close()
    os._exit(0)


signal.signal(signal.SIGINT, signal.SIG_DFL)
time.sleep(float(os.environ.get("FAKE_NOTIFY_SLOW", "0")))
with open(base + ".args.tmp", "w") as f:
    json.dump(sys.argv[1:], f)
os.rename(base + ".args.tmp", base + ".args")
if "--print-id" in sys.argv:
    print(7, flush=True)
signal.signal(signal.SIGINT, closed)
while not os.path.exists(base + ".action"):
    time.sleep(0.02)
time.sleep(0.02)
action = open(base + ".action").read().strip()
if action:
    print(action, flush=True)
