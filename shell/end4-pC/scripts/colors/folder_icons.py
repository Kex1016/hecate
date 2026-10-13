#!/usr/bin/env python3
"""Folder icons in the wallpaper's colour (hecate).

Builds a small icon theme in ~/.local/share/icons that holds Papirus's blue folder icons
recoloured to the current primary colour, inherits everything else from Papirus, and makes
it the GTK and KDE icon theme. The theme alternates between two names: GTK and Qt only reload
an icon theme when its name changes.

Usage: folder_icons.py dark|light
"""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

from materialyoucolor.hct import Hct

from apply_kde_colors import dbus_signal, update_kdeglobals

NAMES = ("Papirus-Hecate", "Papirus-Hecate-Alt")
# Papirus's blue folders: the front, the back flap and the emblem on top.
PAPIRUS_FRONT = "#5294e2"
PAPIRUS_BACK = "#4877b1"
PAPIRUS_EMBLEM = "#1d344f"
# folder-blue.svg, folder-blue-documents.svg, user-blue-home.svg; not folder-bluegrey.svg.
BLUE = re.compile(r"-blue(-[^/]*)?\.svg$")


def home(var, default):
    return Path(os.environ.get(var) or Path.home() / default)


def find_theme(name):
    dirs = [home("XDG_DATA_HOME", ".local/share")]
    dirs += [Path(d) for d in os.environ.get("XDG_DATA_DIRS", "").split(":") if d]
    for d in dirs:
        if (d / "icons" / name / "index.theme").is_file():
            return d / "icons" / name
    return None


def hex_of(argb):
    return "#{:06x}".format(argb & 0xFFFFFF)


def palette(primary):
    """Papirus's three shades (tones 60, 50 and 20) in the primary colour's hue."""
    hct = Hct.from_int(int(primary.lstrip("#"), 16) | 0xFF000000)
    chroma = min(hct.chroma, 60)
    return {
        PAPIRUS_FRONT: hex_of(Hct.from_hct(hct.hue, chroma, 62).to_int()),
        PAPIRUS_BACK: hex_of(Hct.from_hct(hct.hue, chroma, 50).to_int()),
        PAPIRUS_EMBLEM: hex_of(Hct.from_hct(hct.hue, min(chroma, 40), 20).to_int()),
    }


def recolour(svg, colours):
    for old, new in colours.items():
        svg = re.sub(re.escape(old), new, svg, flags=re.IGNORECASE)
    return svg


def build(base, target, name, colours):
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    count = 0
    for size in sorted(p for p in base.iterdir() if (p / "places").is_dir()):
        places = size / "places"
        out = target / size.name / "places"
        out.mkdir(parents=True)
        written = {}
        for entry in sorted(places.iterdir()):
            real = entry.resolve()
            if not BLUE.search(real.name):
                continue
            if real in written:
                # Aliases (folder.svg -> folder-blue.svg) stay aliases.
                (out / entry.name).symlink_to(written[real])
                continue
            (out / entry.name).write_text(recolour(real.read_text(encoding="utf-8"), colours))
            written[real] = entry.name
            count += 1

    index = (base / "index.theme").read_text(encoding="utf-8")
    index = re.sub(r"(?m)^Name=.*$", f"Name={name}", index, count=1)
    index = re.sub(r"(?m)^Inherits=.*$", f"Inherits={base.name},hicolor", index, count=1)
    index = re.sub(r"(?m)^Comment=.*$", "Comment=Papirus with folders in the wallpaper's colour", index, count=1)
    (target / "index.theme").write_text(index)
    return count


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "dark"
    base_name = "Papirus-Dark" if mode == "dark" else "Papirus"
    base = find_theme(base_name)
    if base is None:
        print(f"folder_icons: {base_name} isn't installed, leaving the icon theme alone", file=sys.stderr)
        return

    colors_json = home("XDG_STATE_HOME", ".local/state") / "quickshell/user/generated/colors.json"
    primary = json.loads(colors_json.read_text())["primary"]

    current = subprocess.run(
        ["gsettings", "get", "org.gnome.desktop.interface", "icon-theme"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip().strip("'")
    name = NAMES[1] if current == NAMES[0] else NAMES[0]
    icons = home("XDG_DATA_HOME", ".local/share") / "icons"
    build(base, icons / name, name, palette(primary))

    subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "icon-theme", name], check=False)

    def set_icons(kdeglobals):
        if not kdeglobals.has_section("Icons"):
            kdeglobals.add_section("Icons")
        kdeglobals.set("Icons", "Theme", name)

    update_kdeglobals(set_icons)
    dbus_signal("/KIconLoader", "org.kde.KIconLoader.iconChanged", "int32:0")
    dbus_signal("/KGlobalSettings", "org.kde.KGlobalSettings.notifyChange", "int32:4", "int32:0")
    # The previous theme stays: apps that don't follow the switch keep using it.


if __name__ == "__main__":
    main()
