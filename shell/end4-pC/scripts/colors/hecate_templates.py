#!/usr/bin/env python3
"""App theming from templates (hecate), after Noctalia's.

Renders the current Material colours (material_colors.scss, written by the colour generator)
into app config files:

- built-in templates (templates/<id>.*): foot, kitty. Enabled ids come from the shell config,
  `appearance.wallpaperTheming.appTemplates`. Each writes a colours file next to the app's
  config and adds one include line to that config. A config that's a read-only symlink (Home
  Manager) is left alone, with a warning saying which line to add to it.
- user templates, from ~/.config/hecate/templates.toml:

      [templates.my_app]
      input_path = "~/.config/hecate/templates/my-app.css"
      output_path = "~/.config/my-app/theme.css"   # or a list of paths
      post_hook = "my-app --reload"                # optional, run after writing
      enabled = true                               # optional

Template syntax (a subset of Noctalia's / matugen's):

    {{ colors.<name>.<mode>.<format> [| filter [arg]]... }}   {{ mode }}

<name> is a Material role in snake_case or camelCase (primary, on_primary,
surface_container_high, ...), term0..term15, or a terminal alias (terminal_background,
terminal_foreground, terminal_cursor, terminal_normal_red, terminal_bright_blue, ...).
<mode> is default, dark or light; only the current mode is generated, so all three give the
same colour. <format>: hex, hex_stripped, rgb, rgba, rgb_csv, red, green, blue, alpha.
Filters: set_alpha A, lighten P, darken P, invert, grayscale.

Usage: hecate_templates.py [--only ID]...
"""

import argparse
import colorsys
import json
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
HOME = Path.home()


def xdg(var, default):
    return Path(os.environ.get(var) or HOME / default)


CONFIG = xdg("XDG_CONFIG_HOME", ".config")
STATE = xdg("XDG_STATE_HOME", ".local/state")
SCSS = STATE / "quickshell/user/generated/material_colors.scss"
SHELL_CONFIG = CONFIG / "illogical-impulse/config.json"
USER_TEMPLATES = CONFIG / "hecate/templates.toml"

# id -> (template file, output, app config, include syntax). The include goes first, so the
# user's own colours in that config still win.
BUILTIN = {
    "foot": ("foot.ini", CONFIG / "foot/hecate-colors.ini", CONFIG / "foot/foot.ini", "include={}"),
    "kitty": ("kitty.conf", CONFIG / "kitty/hecate-colors.conf", CONFIG / "kitty/kitty.conf", "include {}"),
}
# Run after writing a built-in template. Open foot windows get their colours from the
# escape sequences applycolor.sh sends to every terminal.
BUILTIN_HOOKS = {"kitty": "pkill -USR1 -x kitty"}
# Same order as scripts/colors/terminal/sequences.txt.
TERMINAL_NAMES = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]


def warn(msg):
    print(f"hecate_templates: {msg}", file=sys.stderr)


class Color:
    def __init__(self, r, g, b, a=1.0):
        self.r, self.g, self.b, self.a = r, g, b, a

    @classmethod
    def parse(cls, text):
        text = text.strip().lstrip("#")
        return cls(int(text[0:2], 16), int(text[2:4], 16), int(text[4:6], 16))

    def _with_lightness(self, f):
        h, l, s = colorsys.rgb_to_hls(self.r / 255, self.g / 255, self.b / 255)
        r, g, b = colorsys.hls_to_rgb(h, min(1.0, max(0.0, f(l))), s)
        return Color(round(r * 255), round(g * 255), round(b * 255), self.a)

    def filter(self, name, arg):
        if name == "set_alpha":
            return Color(self.r, self.g, self.b, float(arg))
        if name == "lighten":
            return self._with_lightness(lambda l: l + float(arg) / 100)
        if name == "darken":
            return self._with_lightness(lambda l: l - float(arg) / 100)
        if name == "invert":
            return Color(255 - self.r, 255 - self.g, 255 - self.b, self.a)
        if name == "grayscale":
            y = round(0.299 * self.r + 0.587 * self.g + 0.114 * self.b)
            return Color(y, y, y, self.a)
        raise ValueError(f"unknown filter {name!r}")

    def format(self, fmt):
        hex6 = f"{self.r:02x}{self.g:02x}{self.b:02x}"
        alpha = f"{self.a:g}"
        return {
            "hex": f"#{hex6}",
            "hex_stripped": hex6,
            "rgb": f"rgb({self.r}, {self.g}, {self.b})",
            "rgba": f"rgba({self.r}, {self.g}, {self.b}, {alpha})",
            "rgb_csv": f"{self.r},{self.g},{self.b}",
            "red": str(self.r),
            "green": str(self.g),
            "blue": str(self.b),
            "alpha": alpha,
        }[fmt]


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def load_colors():
    colors, dark = {}, True
    for line in SCSS.read_text().splitlines():
        m = re.match(r"^\$(\w+):\s*(\S+);", line)
        if not m:
            continue
        name, value = m.groups()
        if name == "darkmode":
            dark = value == "True"
        elif re.fullmatch(r"#[0-9A-Fa-f]{6}", value):
            colors[name] = colors[snake(name)] = Color.parse(value)
    if "term0" in colors:
        colors["terminal_background"] = colors["term0"]
        colors["terminal_foreground"] = colors["terminal_cursor"] = colors["term7"]
        for i, n in enumerate(TERMINAL_NAMES):
            colors[f"terminal_normal_{n}"] = colors[f"term{i}"]
            colors[f"terminal_bright_{n}"] = colors[f"term{i + 8}"]
    return colors, "dark" if dark else "light"


EXPR = re.compile(r"\{\{\s*(.*?)\s*\}\}")


def render(text, colors, mode):
    def expr(m):
        parts = [p.strip() for p in m.group(1).split("|")]
        head, filters = parts[0], parts[1:]
        if head == "mode":
            return mode
        path = head.split(".")
        if len(path) != 4 or path[0] != "colors" or path[2] not in ("default", "dark", "light"):
            raise ValueError(f"can't read {head!r} (expected colors.<name>.<mode>.<format>)")
        color = colors.get(path[1])
        if color is None:
            raise ValueError(f"unknown colour {path[1]!r}")
        for f in filters:
            name, _, arg = f.replace(":", " ", 1).partition(" ")
            color = color.filter(name.strip(), arg.strip())
        return color.format(path[3])

    return EXPR.sub(expr, text)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text)
    os.replace(tmp, path)


def ensure_include(config, output, syntax, app):
    line = syntax.format(output)
    if config.is_symlink() and not os.access(config.resolve(), os.W_OK):
        if str(output) not in config.read_text():
            warn(f"{config} is read-only (Home Manager?); add `{line}` to it for {app} colours")
        return
    if config.exists():
        text = config.read_text()
        if str(output) in text:
            return
    else:
        # A new user config would hide the system one (NixOS's /etc/xdg/...): keep it.
        system = Path("/etc/xdg") / config.relative_to(CONFIG)
        text = syntax.format(system) + "\n" if system.exists() else ""
    write(config, f"{line}\n{text}")


def run_hook(cmd):
    if cmd:
        subprocess.Popen(["sh", "-c", cmd], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def enabled_builtins():
    try:
        theming = json.loads(SHELL_CONFIG.read_text())["appearance"]["wallpaperTheming"]
        return theming.get("appTemplates", ["foot"])
    except (OSError, KeyError, ValueError):
        return ["foot"]


def user_templates():
    if not USER_TEMPLATES.exists():
        return {}
    try:
        return tomllib.loads(USER_TEMPLATES.read_text()).get("templates", {})
    except tomllib.TOMLDecodeError as e:
        warn(f"{USER_TEMPLATES}: {e}")
        return {}


def expand(p):
    return Path(os.path.expandvars(os.path.expanduser(p)))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--only", action="append", help="render just these template ids")
    args = parser.parse_args()
    if not SCSS.exists():
        warn(f"{SCSS} doesn't exist yet; pick a wallpaper first")
        return 1
    colors, mode = load_colors()
    wanted = lambda i: not args.only or i in args.only
    status = 0

    for tid in enabled_builtins():
        if tid not in BUILTIN:
            warn(f"unknown built-in template {tid!r} (have: {', '.join(BUILTIN)})")
            continue
        if not wanted(tid):
            continue
        template, output, config, syntax = BUILTIN[tid]
        try:
            write(output, render((SCRIPT_DIR / "templates" / template).read_text(), colors, mode))
            ensure_include(config, output, syntax, tid)
            run_hook(BUILTIN_HOOKS.get(tid))
        except (OSError, ValueError) as e:
            warn(f"{tid}: {e}")
            status = 1

    for tid, t in user_templates().items():
        if not wanted(tid) or not t.get("enabled", True):
            continue
        try:
            text = render(expand(t["input_path"]).read_text(), colors, mode)
            outputs = t["output_path"]
            for out in [outputs] if isinstance(outputs, str) else outputs:
                write(expand(out), text)
            run_hook(t.get("post_hook"))
        except (OSError, ValueError, KeyError) as e:
            warn(f"{tid}: {e}")
            status = 1
    return status


if __name__ == "__main__":
    sys.exit(main())
