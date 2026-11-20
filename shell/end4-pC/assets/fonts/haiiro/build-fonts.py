#!/usr/bin/env python3
"""Rebuild the haiiro font files from their upstream downloads.

    build-fonts.py <downloads dir> [<output dir>]

<downloads dir> holds the upstream files exactly as fetched (see SOURCES below and
README.md). The output dir defaults to the directory of this script.

What it does:
- IBM VGA 8x16: takes "PxPlus IBM VGA 8x16" and its double-width "-2x" variant from the
  Ultimate Oldschool PC Font Pack and registers them as one family, "IBM VGA 8x16", with
  the 2x face as its bold (weight 700). Only the name, OS/2 and head tables change; the
  glyphs, metrics and character map are untouched.
- Jacquard 24, Unifont and Unifont Upper are copied byte for byte.
- Every face also gets a .woff2 copy in web/, for use on web pages.

Needs fontTools and brotli (pip install fonttools brotli).
"""

import hashlib
import shutil
import sys
import zipfile
from pathlib import Path

from fontTools.ttLib import TTFont

SOURCES = {
    # file name in the downloads dir: (url, sha256)
    "oldschool_pc_font_pack_v2.2_FULL.zip": (
        "https://int10h.org/oldschool-pc-fonts/download/oldschool_pc_font_pack_v2.2_FULL.zip",
        "21b3c0a3770ef0afc46564760613d7b078f4fcc9ed93db4b829a440b68822e08",
    ),
    "Jacquard24-Regular.ttf": (
        "https://raw.githubusercontent.com/google/fonts/main/ofl/jacquard24/Jacquard24-Regular.ttf",
        "8e5b1bdb8b6087d6175f1d9edd2634e38b10fb109c0955c24ad55272a866ed49",
    ),
    "unifont-18.0.01.otf": (
        "https://unifoundry.com/pub/unifont/unifont-18.0.01/font-builds/unifont-18.0.01.otf",
        "88d0a14d4aa9a96419720b39ba5da921a59560d76d4ec7d523acc06ed85cd3c2",
    ),
    "unifont_upper-18.0.01.otf": (
        "https://unifoundry.com/pub/unifont/unifont-18.0.01/font-builds/unifont_upper-18.0.01.otf",
        "472201e45a050bff4c7658208b8c73e34ebce3764b287a5a46c7774038ef9391",
    ),
}

FAMILY = "IBM VGA 8x16"
PACK = "oldschool_pc_font_pack_v2.2_FULL.zip"
VGA_FACES = {
    # member of the font pack: (style name, weight, output file)
    "ttf - Px (pixel outline)/PxPlus_IBM_VGA_8x16.ttf": ("Regular", 400, "IBMVGA8x16-Regular.ttf"),
    "ttf - Px (pixel outline)/PxPlus_IBM_VGA_8x16-2x.ttf": ("Bold", 700, "IBMVGA8x16-Bold.ttf"),
}
COPIED = {
    "Jacquard24-Regular.ttf": "Jacquard24-Regular.ttf",
    "unifont-18.0.01.otf": "Unifont.otf",
    "unifont_upper-18.0.01.otf": "UnifontUpper.otf",
}


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def check_sources(downloads):
    for name, (url, want) in SOURCES.items():
        path = downloads / name
        if not path.exists():
            sys.exit(f"missing {path}\n  fetch it from {url}")
        got = sha256(path)
        if got != want:
            sys.exit(f"{path}: sha256 is {got}, expected {want}")


def build_vga(pack, member, style, weight, out):
    with pack.open(member) as src:
        font = TTFont(src)
        font.ensureDecompiled()
    original = font["name"].getDebugName(4)
    version = font["name"].getDebugName(5)
    postscript = f"{FAMILY.replace(' ', '')}-{style}"
    full = FAMILY if style == "Regular" else f"{FAMILY} {style}"
    names = {
        1: FAMILY,
        2: style,
        3: f"haiiro:{full}:{version}",
        4: full,
        6: postscript,
        10: f"{original} from The Ultimate Oldschool PC Font Pack v2.2 by VileR (int10h.org), "
        f'registered as "{FAMILY}" {style} for the hecate shell. Glyphs unchanged.',
    }
    for name_id, text in names.items():
        # Windows/Unicode and Macintosh/Roman records, as upstream has them.
        font["name"].setName(text, name_id, 3, 1, 0x409)
        font["name"].setName(text, name_id, 1, 0, 0)

    bold = weight >= 700
    os2 = font["OS/2"]
    os2.usWeightClass = weight
    # fsSelection: bit 5 = bold, bit 6 = regular.
    os2.fsSelection = (os2.fsSelection & ~0b1100000) | (0b0100000 if bold else 0b1000000)
    font["head"].macStyle = (font["head"].macStyle & ~1) | (1 if bold else 0)
    # FontForge's own timestamp table; nothing reads it.
    if "FFTM" in font:
        del font["FFTM"]
    font.save(out)


def write_woff2(src, out):
    font = TTFont(src)
    font.flavor = "woff2"
    font.save(out)


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    downloads = Path(sys.argv[1])
    out = Path(sys.argv[2]) if len(sys.argv) == 3 else Path(__file__).resolve().parent
    web = out / "web"
    web.mkdir(parents=True, exist_ok=True)
    check_sources(downloads)

    built = []
    with zipfile.ZipFile(downloads / PACK) as pack:
        for member, (style, weight, name) in VGA_FACES.items():
            build_vga(pack, member, style, weight, out / name)
            built.append(name)
    for src, name in COPIED.items():
        shutil.copyfile(downloads / src, out / name)
        built.append(name)
    for name in built:
        write_woff2(out / name, web / (Path(name).stem + ".woff2"))

    for path in sorted([out / n for n in built] + list(web.glob("*.woff2"))):
        print(f"{sha256(path)}  {path.relative_to(out)}")


if __name__ == "__main__":
    main()
