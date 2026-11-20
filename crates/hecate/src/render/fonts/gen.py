#!/usr/bin/env python3
"""Regenerate the bitmap font data next to this script. See README.md for the sources.

    python3 gen.py --vga PxPlus_IBM_VGA_8x16.ttf \
        --unifont unifont_all-18.0.01.hex --combining unifont-combining-18.0.01.txt

Needs fontTools and Pillow. Writes vga8x16.bin, unifont.idx and unifont.png.
"""

import argparse
import struct
from pathlib import Path

from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent

# Scripts drawn right to left, or that need shaping, reordering or conjuncts: a row of
# their glyphs in logical order is not their text, so they stay out and a title that uses
# them falls back to the app name. Controls, surrogates and private use are out too.
EXCLUDED = [
    (0x0000, 0x001F),
    (0x007F, 0x009F),
    (0x0590, 0x08FF),  # Hebrew, Arabic, Syriac, Thaana, NKo, Samaritan, Mandaic
    (0x0900, 0x0DFF),  # Indic scripts
    (0x0F00, 0x109F),  # Tibetan, Myanmar
    (0x1100, 0x11FF),  # conjoining Hangul jamo
    (0x1780, 0x18AF),  # Khmer, Mongolian
    (0x1900, 0x1AAF),  # Limbu to Tai Tham
    (0x1B00, 0x1C4F),  # Balinese to Lepcha
    (0x1CD0, 0x1CFF),  # Vedic extensions
    (0xA800, 0xA8FF),  # Syloti Nagri, Phags-pa, Saurashtra, Devanagari extended
    (0xA930, 0xA9FF),  # Rejang, Hangul jamo extended A, Javanese, Myanmar extended B
    (0xAA00, 0xAAFF),  # Cham to Meetei Mayek extensions
    (0xABC0, 0xABFF),  # Meetei Mayek
    (0xD7B0, 0xF8FF),  # Hangul jamo extended B, surrogates, private use
    (0xFB1D, 0xFDFF),  # Hebrew and Arabic presentation forms
    (0xFE70, 0xFEFF),  # Arabic presentation forms B, BOM
    (0xFFF0, 0xFFFC),  # specials
]
# Beyond the BMP only these go in: kana, mathematical alphanumerics ("fancy" Latin),
# emoji and symbols, and the CJK extensions.
INCLUDED_ABOVE_BMP = [
    (0x1B000, 0x1B16F),
    (0x1D400, 0x1D7FF),
    (0x1F000, 0x1FBFF),
    (0x20000, 0x3FFFF),
]

FLAG_WIDE = 1
FLAG_COMBINING = 2


def wanted(cp):
    if cp >= 0x10000:
        return any(a <= cp <= b for a, b in INCLUDED_ABOVE_BMP)
    return not any(a <= cp <= b for a, b in EXCLUDED)


def vga_glyphs(path):
    """{codepoint: 16 bytes, one per row, bit 7 the leftmost pixel}."""
    cmap = TTFont(path).getBestCmap()
    font = ImageFont.truetype(str(path), 16)
    out = {}
    for cp in sorted(cmap):
        if cp < 0x20 or not wanted(cp):
            continue
        im = Image.new("1", (8, 16), 0)
        draw = ImageDraw.Draw(im)
        draw.fontmode = "1"
        draw.text((0, 0), chr(cp), font=font, fill=1)
        out[cp] = bytes(
            sum(1 << (7 - x) for x in range(8) if im.getpixel((x, y))) for y in range(16)
        )
    return out


def unifont_glyphs(path, skip):
    """{codepoint: 16 bytes (8 px wide) or 32 bytes (16 px wide, two per row)}."""
    out = {}
    for line in Path(path).read_text().splitlines():
        cp, _, bits = line.partition(":")
        cp = int(cp, 16)
        if len(bits) not in (32, 64) or cp in skip or cp < 0x20 or not wanted(cp):
            continue
        out[cp] = bytes.fromhex(bits)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--vga", required=True)
    ap.add_argument("--unifont", required=True)
    ap.add_argument("--combining", required=True)
    ap.add_argument("--out", default=str(HERE))
    args = ap.parse_args()
    out = Path(args.out)

    vga = vga_glyphs(args.vga)
    with open(out / "vga8x16.bin", "wb") as f:
        for cp, rows in sorted(vga.items()):
            f.write(struct.pack("<I", cp) + rows)

    combining = {}
    for line in Path(args.combining).read_text().splitlines():
        cp, _, offset = line.partition(":")
        combining[int(cp, 16)] = int(offset)
    uni = unifont_glyphs(args.unifont, vga)

    # The picture is a strip of 8x16 cells, 256 to a row; a wide glyph is two cells in a
    # row (wrapping to the next line when it must). The index lists runs of consecutive
    # code points of one kind, in the order their cells appear.
    cells = []
    runs = []
    for cp in sorted(uni):
        rows = uni[cp]
        wide = len(rows) == 32
        flags = (FLAG_WIDE if wide else 0) | (FLAG_COMBINING if cp in combining else 0)
        offset = combining.get(cp, 0)
        if wide:
            cells.append(rows[0::2])
            cells.append(rows[1::2])
        else:
            cells.append(rows)
        last = runs[-1] if runs else None
        if last and last[0] + last[1] == cp and last[2:] == [flags, offset] and last[1] < 0xFFFF:
            last[1] += 1
        else:
            runs.append([cp, 1, flags, offset])
    with open(out / "unifont.idx", "wb") as f:
        for start, count, flags, offset in runs:
            f.write(struct.pack("<IHBb", start, count, flags, offset))

    per_row = 256
    height = -(-len(cells) // per_row) * 16
    raw = bytearray(per_row * height)
    for i, cell in enumerate(cells):
        x, y = i % per_row, (i // per_row) * 16
        for r in range(16):
            raw[(y + r) * per_row + x] = cell[r]
    im = Image.frombytes("1", (per_row * 8, height), bytes(raw))
    im.save(out / "unifont.png", optimize=True)

    print(f"vga8x16.bin: {len(vga)} glyphs")
    print(f"unifont: {len(uni)} glyphs in {len(runs)} runs, {len(cells)} cells")


if __name__ == "__main__":
    main()
