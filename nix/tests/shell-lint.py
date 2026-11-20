"""Static checks on the bundled shell's sources, with no display and no inputs fetched.

Usage: shell-lint.py <shell dir> <qmllint> <node>

- qmllint reports syntax errors and structural mistakes (duplicate bindings, inheritance
  cycles). Warnings about types it cannot resolve are not gated: without a qmldir tree for the
  `qs.` imports it reports thousands, and the shell's own ones are only meaningful at runtime.
- every `import qs.<dir>` names a directory that exists in the shell
- every `.js` file parses (a leading `.pragma library` is stripped first, as it is not JS)
"""

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

shell = Path(sys.argv[1])
qmllint, node = sys.argv[2], sys.argv[3]

# Rules whose findings are real defects wherever they appear; the rest is import noise.
GATED = {
    "syntax",
    "duplicate-property-binding",
    "duplicated-name",
    "inheritance-cycle",
    "duplicate-enum-entries",
    "duplicate-import",
    "unresolved-singleton",
}

failures = []
qml = sorted(shell.rglob("*.qml"))
assert qml, f"no .qml files under {shell}"

with tempfile.TemporaryDirectory() as tmp:
    out = Path(tmp, "lint.json")
    env = dict(os.environ, QT_QPA_PLATFORM="offscreen")
    subprocess.run(
        [qmllint, "--ignore-settings", "--json", str(out), *map(str, qml)],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    report = json.loads(out.read_text())
    linted = {f["filename"] for f in report["files"]}
    for f in report["files"]:
        for w in f["warnings"]:
            if w.get("type") == "error" or w.get("id") in GATED:
                failures.append(f"{f['filename']}:{w.get('line', '?')}: [{w.get('id')}] {w['message']}")
    missed = [str(p) for p in qml if str(p) not in linted]
    if missed:
        failures.append(f"qmllint skipped {len(missed)} files, first: {missed[0]}")

    # Resolve `import qs.a.b` against the shell's directory tree.
    pat = re.compile(r"^\s*import\s+qs\.([\w.]+)", re.M)
    for p in qml:
        for m in pat.finditer(p.read_text()):
            target = shell.joinpath(*m.group(1).split("."))
            if not target.is_dir():
                failures.append(f"{p}: import qs.{m.group(1)} has no directory")

    for p in sorted(shell.rglob("*.js")):
        src = re.sub(r"^\s*\.(pragma|import)\b.*$", "", p.read_text(), flags=re.M)
        tmp_js = Path(tmp, "check.mjs" if re.search(r"^\s*(import|export)\s", src, re.M) else "check.js")
        tmp_js.write_text(src)
        r = subprocess.run([node, "--check", str(tmp_js)], capture_output=True, text=True)
        if r.returncode:
            failures.append(f"{p}: {r.stderr.strip().splitlines()[0] if r.stderr.strip() else 'does not parse'}")

if failures:
    print("\n".join(failures))
    sys.exit(f"{len(failures)} problem(s)")
print(f"{len(qml)} qml files ok")
