#!/usr/bin/env python3
"""Apply a KDE colour scheme without Plasma (hecate).

plasma-apply-colorscheme ships with plasma-workspace, which a hecate session doesn't have.
This does what it does for Qt apps: copy the scheme's colour groups into ~/.config/kdeglobals
and tell running KDE/Qt apps (through plasma-integration) to reload their palette.

Usage: apply_kde_colors.py SCHEME.colors
"""

import configparser
import os
import subprocess
import sys
from pathlib import Path

# The groups plasma-apply-colorscheme copies into kdeglobals.
COPIED_PREFIXES = ("Colors:", "ColorEffects:")
COPIED_SECTIONS = ("WM",)


def ini():
    parser = configparser.ConfigParser(interpolation=None, strict=False)
    parser.optionxform = str
    return parser


def update_kdeglobals(edit):
    """Read ~/.config/kdeglobals, let `edit` change it, write it back."""
    config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    kdeglobals_path = config_home / "kdeglobals"
    kdeglobals = ini()
    kdeglobals.read(kdeglobals_path, encoding="utf-8")
    edit(kdeglobals)
    # Replace, never write through: a read-only symlink (Home Manager) is swapped for a file.
    tmp = kdeglobals_path.with_name(kdeglobals_path.name + ".tmp")
    with open(tmp, "w", encoding="utf-8") as f:
        kdeglobals.write(f, space_around_delimiters=False)
    os.replace(tmp, kdeglobals_path)


def dbus_signal(path, member, *args):
    subprocess.run(
        ["dbus-send", "--session", "--type=signal", path, member, *args],
        check=False,
    )


def copy_scheme(scheme, scheme_path, kdeglobals):
    for section in scheme.sections():
        if not (section.startswith(COPIED_PREFIXES) or section in COPIED_SECTIONS):
            continue
        if kdeglobals.has_section(section):
            kdeglobals.remove_section(section)
        kdeglobals.add_section(section)
        for key, value in scheme.items(section):
            kdeglobals.set(section, key, value)

    if not kdeglobals.has_section("General"):
        kdeglobals.add_section("General")
    kdeglobals.set("General", "ColorScheme", scheme_path.stem)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__.strip().splitlines()[-1])
    scheme_path = Path(sys.argv[1])
    scheme = ini()
    scheme.read(scheme_path, encoding="utf-8")
    update_kdeglobals(lambda kdeglobals: copy_scheme(scheme, scheme_path, kdeglobals))
    # KGlobalSettings::PaletteChanged; plasma-integration reloads kdeglobals on it.
    dbus_signal("/KGlobalSettings", "org.kde.KGlobalSettings.notifyChange", "int32:0", "int32:0")


if __name__ == "__main__":
    main()
