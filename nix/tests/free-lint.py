"""Static checks on the kit and the free widgets, with no display.

Usage: free-lint.py <shell dir> <qmllint> <qt qml dir> <quickshell qml dir>

shell-lint.py gates only the rules that hold without type information. This one resolves the
shell's `qs.` imports (a qmldir is generated for every directory of a copy of the shell, singletons
included), so qmllint also sees a misspelled property on a kit component, a type that does not
exist, an unqualified name, or a call into a service that is not there. It runs over the kit and the
free widgets only, and fails on every warning that is not in IGNORED below.

- the catalogue in services/DesktopWidgets.qml and free/ agree: every `free:` path is a file, every
  file in free/ is in the catalogue, every `file:` path is a file, and `free/<Name>Free.qml` is named
  after its window file
- every free widget's root is `H.FreeWidget`, imported from the kit
- the kit imports only Qt, Quickshell and itself (HTokens also reads the token source, which the
  installer stands in for)
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

shell = Path(sys.argv[1])
qmllint, qt_qml, qs_qml = sys.argv[2], sys.argv[3], sys.argv[4]

KIT = shell / "modules/common/widgets/haiiro"
FREE = shell / "modules/ii/background/widgets/free"
WIDGETS = shell / "modules/ii/background/widgets"
CATALOG = shell / "services/DesktopWidgets.qml"

# Not defects, or defects that are somebody else's and written down. (id, file name or None,
# message pattern). Anything else qmllint says about the kit or a free file fails the check.
IGNORED = [
    # style, not behaviour
    ("unused-imports", None, r"."),
    ("id-shadows-member", None, r"."),
    ("confusing-expression-statement", None, r"."),
    # qmllint reads the qmldir entry and the file's own `pragma Singleton` as two declarations
    ("import", None, r"not declared as singleton in qmldir"),
    # Quickshell's JsonObject has no properties of its own, so a nested Config option is unknown
    ("missing-property", None, r'not found on type "qs::io::JsonObject"'),
    # Drive does not report a quota yet; the widget reads it with an index and tolerates undefined
    ("missing-property", "AppleDriveFree.qml", r'Member "quota" not found on type "Drive"'),
]

failures = []


def fail(msg):
    failures.append(msg)


# --- catalogue and free/ ---------------------------------------------------------------------
cat = CATALOG.read_text()
entries = re.findall(r'\{\s*key:.*?file:\s*"([^"]+)"(?:,\s*free:\s*"([^"]+)")?', cat)
if len(entries) < 28:
    fail(f"{CATALOG.name}: found {len(entries)} catalogue entries, expected at least 28")
listed = set()
for window, free in entries:
    if not (WIDGETS / window).is_file():
        fail(f"catalogue: window file {window} does not exist")
    if not free:
        fail(f"catalogue: {window} has no free form")
        continue
    listed.add(free)
    if not (WIDGETS / free).is_file():
        fail(f"catalogue: free file {free} does not exist")
    expect = "free/" + Path(window).name.replace("Widget.qml", "") + "Free.qml"
    if free != expect:
        fail(f"catalogue: {window} names {free}, the naming rule gives {expect}")
for p in sorted(FREE.glob("*.qml")):
    if f"free/{p.name}" not in listed:
        fail(f"{p.relative_to(shell)} is in no catalogue entry")

root_pat = re.compile(r"^\s*H\.FreeWidget\s*\{", re.M)
for p in sorted(FREE.glob("*.qml")):
    text = p.read_text()
    if not re.search(r"^import qs\.modules\.common\.widgets\.haiiro as H\s*$", text, re.M):
        fail(f"{p.relative_to(shell)}: does not import the kit as H")
    body = re.sub(r"^\s*(//.*|import .*|pragma .*)$", "", text, flags=re.M).lstrip()
    if not root_pat.match(body):
        fail(f"{p.relative_to(shell)}: root is not H.FreeWidget")

# --- the kit imports nothing outside itself ------------------------------------------------------
OK_IMPORT = re.compile(r"^(QtQuick(\.\w+)*|Quickshell|qs\.modules\.common\.widgets\.haiiro)(\s+as\s+\w+)?$")
KIT_EXCEPTIONS = {"HTokens.qml": {"qs.modules.common"}}
for p in sorted(KIT.glob("*.qml")):
    for m in re.finditer(r"^import\s+(\S.*?)\s*$", p.read_text(), re.M):
        spec = m.group(1)
        if not OK_IMPORT.match(spec) and spec not in KIT_EXCEPTIONS.get(p.name, ()):
            fail(f"{p.relative_to(shell)}: imports {spec}, which is outside the kit")

# --- qmllint with the `qs.` imports resolved ---------------------------------------------------
with tempfile.TemporaryDirectory() as tmp:
    tree = Path(tmp, "tree")
    shutil.copytree(shell, tree / "qs", symlinks=True)
    for p in [tree / "qs", *(tree / "qs").rglob("*")]:
        if not p.is_symlink():
            p.chmod(p.stat().st_mode | 0o700)
    for d in [tree / "qs", *[p for p in (tree / "qs").rglob("*") if p.is_dir()]]:
        rel = d.relative_to(tree / "qs").parts
        lines = ["module " + ".".join(("qs", *rel))]
        for f in sorted(d.glob("*.qml")):
            if f.name[0].isupper():
                single = re.search(r"^\s*pragma\s+Singleton", f.read_text(), re.M)
                lines.append(f"{'singleton ' if single else ''}{f.stem} 1.0 {f.name}")
        if len(lines) > 1:
            (d / "qmldir").write_text("\n".join(lines) + "\n")

    mine = [tree / "qs" / p.relative_to(shell) for p in [*sorted(KIT.glob("*.qml")), *sorted(FREE.glob("*.qml"))]]
    out = Path(tmp, "lint.json")
    subprocess.run(
        [qmllint, "--ignore-settings", "-I", str(tree), "-I", qt_qml, "-I", qs_qml, "--json", str(out), *map(str, mine)],
        env=dict(os.environ, QT_QPA_PLATFORM="offscreen"),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if not out.exists():
        fail("qmllint wrote no report")
    else:
        report = json.loads(out.read_text())
        seen = {f["filename"] for f in report["files"]}
        for f in report["files"]:
            name = Path(f["filename"]).name
            for w in f["warnings"]:
                wid, msg = w.get("id", ""), w.get("message", "")
                if any(i == wid and (n is None or n == name) and re.search(pat, msg) for i, n, pat in IGNORED):
                    continue
                # Notes that follow a warning (the other half of a shadowed id) carry no id of their own.
                if msg.startswith("Note:"):
                    continue
                fail(f"{Path(f['filename']).relative_to(tree / 'qs')}:{w.get('line', '?')}: [{wid}] {msg}")
        missed = [str(p) for p in mine if str(p) not in seen]
        if missed:
            fail(f"qmllint skipped {len(missed)} files, first: {missed[0]}")

if failures:
    print("\n".join(failures))
    sys.exit(f"{len(failures)} problem(s)")
print(f"{len(entries)} catalogue entries, {len(list(FREE.glob('*.qml')))} free widgets, {len(list(KIT.glob('*.qml')))} kit files ok")
