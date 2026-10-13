# Self-check for hecate_templates.py's editor calls (--list, --preview, --save).
# Run: python3 -I test_hecate_templates.py
import json, os, subprocess, sys, tempfile
from pathlib import Path
S = str(Path(__file__).resolve().parent / "hecate_templates.py")
T = tempfile.mkdtemp()
gen = Path(T, "state/quickshell/user/generated")
gen.mkdir(parents=True)
gen.joinpath("material_colors.scss").write_text("$darkmode: True;\n$primary: #aabbcc;\n$onPrimary: #112233;\n"
    + "".join(f"$term{i}: #0000{i:02x};\n" for i in range(16)))
env = dict(os.environ, XDG_CONFIG_HOME=f"{T}/cfg", XDG_STATE_HOME=f"{T}/state")
run = lambda *a, inp="": subprocess.run([sys.executable, "-I", S, *a], input=inp, capture_output=True, text=True, env=env)
r = run("--preview", inp="c={{ colors.primary.default.hex }} m={{ mode }}")
assert r.returncode == 0 and r.stdout == "c=#aabbcc m=dark", r
assert run("--preview", inp="{{ colors.nope.default.hex }}").returncode == 1
assert run("--save", "bad name", f"{T}/out.css", inp="x").returncode == 1
r = run("--save", "my-app", f"{T}/out.css", inp="a {{ colors.on_primary.default.hex_stripped }}")
assert r.returncode == 0, r
assert open(f"{T}/out.css").read() == "a 112233"
r = run("--save", "my-app", f"{T}/out.css", inp="b")  # existing: rewritten, not registered twice
assert open(f"{T}/out.css").read() == "b" and open(f"{T}/cfg/hecate/templates.toml").read().count("[templates.my-app]") == 1
d = json.loads(run("--list").stdout)
assert d["templates"]["my-app"]["output"] == f"{T}/out.css" and d["colors"]["primary"] == "#aabbcc" and "onPrimary" not in d["colors"], d
print("self-check ok")
