"""Check that docs/keybinds.md's shell table lists exactly the shell profile's binds.

Usage: keybind-docs.py docs/keybinds.md binds.json   (binds.json: the profile's key names)

Notation in the bind column: `Mod+A/B` is one combo each; after ` / `, a bare `B` keeps the
previous combo's modifiers (`Mod+Shift+N` / `B` is Mod+Shift+B); after `, ` it stands alone.
"""

import json
import re
import sys


def documented(cell):
    out = []
    prefix = ""
    for sep, tok in re.findall(r"(^|\s*/\s*|\s*,\s*|[^`]*?)`([^`]+)`", cell):
        if "+" not in tok and "/" in sep:
            tok = prefix + tok
        pre, _, last = tok.rpartition("+")
        prefix = pre + "+" if pre else ""
        out.extend(prefix + k for k in last.split("/"))
    return out


def norm(combo):
    return combo.lower()


def main():
    doc = open(sys.argv[1]).read()
    binds = {norm(b) for b in json.load(open(sys.argv[2]))}
    section = next(s for s in doc.split("\n## ") if s.startswith("shell"))
    docs = set()
    for line in section.splitlines():
        if line.startswith("| `"):
            docs.update(norm(c) for c in documented(line.split("|")[1]))

    errors = [f"docs list {d!r}, which the shell profile doesn't bind" for d in sorted(docs - binds)]
    errors += [f"the shell profile binds {b!r}, missing from the docs" for b in sorted(binds - docs)]
    for e in errors:
        print(e, file=sys.stderr)
    if errors:
        sys.exit(1)
    print(f"{len(binds)} shell binds documented")


if __name__ == "__main__":
    main()
