# Label fonts

The bitmap faces the window label is drawn with (`../label.rs`), compiled into the binary.
Both sit on an 8x16 grid, so one line can mix them.

| File | What | From |
| --- | --- | --- |
| `vga8x16.bin` | IBM VGA 8x16, 715 glyphs: Latin (with Latin-1 and Extended-A), Greek, Cyrillic, box drawing, the CP437 symbols | "PxPlus IBM VGA 8x16" in The Ultimate Oldschool PC Font Pack v2.2 |
| `unifont.idx`, `unifont.png` | GNU Unifont 18.0.01, 62,754 glyphs: what the VGA face lacks, CJK, Hangul, kana, symbols and emoji among them | `unifont_all-18.0.01.hex` (Unifont and Unifont Upper), `unifont-combining-18.0.01.txt` |

A character is looked up in the VGA face first and in Unifont second. A title with a
character in neither is not drawn: the label then shows the app name alone.

## Licences

- **The Ultimate Oldschool PC Font Pack** by VileR, <https://int10h.org/oldschool-pc-fonts/>,
  is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).
  `vga8x16.bin` is the pack's "PxPlus IBM VGA 8x16" converted to a table of bitmaps, so it is
  under the same licence. "Plus" is the pack's extended character set of the same face that
  the design system's `Web437_IBM_VGA_8x16` is the CP437 set of; it adds the accented Latin
  and the Cyrillic that window titles need, and draws 11 Greek letters as letters rather than
  as CP437's maths symbols.
- **GNU Unifont** by Roman Czyborra, Paul Hardy and contributors,
  <https://unifoundry.com/unifont/>, is dual-licensed under the GNU GPL version 2 or later
  with the GNU Font Embedding Exception, and under the SIL Open Font License 1.1.
  `unifont.idx` and `unifont.png` are its glyphs in another container.

## Formats

- `vga8x16.bin`: records of 20 bytes, sorted by code point: the code point (u32, little
  endian) and 16 rows, one byte each, leftmost pixel in the high bit.
- `unifont.idx`: runs of consecutive code points of one kind, 8 bytes each, sorted: first
  code point (u32, little endian), length (u16), flags (1: wide, two cells a glyph; 2:
  combining), and for combining marks the offset from the pen they are drawn at (i8).
- `unifont.png`: a 1-bit picture, 2048 px wide: 256 cells of 8x16 to a row, in the order of
  the runs. PNG because it packs the 1.9 MB of bitmaps into 1.1 MB; it is decoded once, when
  a title first needs a glyph from it.

## What is left out of Unifont

Drawing glyph after glyph from left to right only works for scripts written that way.
Scripts that need shaping or run right to left would come out wrong, which is worse than not
at all, so `gen.py` drops them (`EXCLUDED`): Hebrew, Arabic, Syriac, Thaana, N'Ko, the Indic
scripts, Tibetan, Myanmar, Khmer, Mongolian and their kin, conjoining Hangul jamo (precomposed
Hangul is in), control characters, surrogates and private use. Above the BMP only kana
extensions, mathematical alphanumerics, the symbol and emoji blocks and the CJK extensions
are taken (`INCLUDED_ABOVE_BMP`). Glyphs the VGA face has are left out too.

## Regenerating

```sh
# https://int10h.org/oldschool-pc-fonts/download/oldschool_pc_font_pack_v2.2_linux.zip
#   sha256 b30dc3ecc9931ad2dd8be7517dd01813c8834a1911b582ab7643191b41a3d759
#   -> "ttf - Px (pixel outline)/PxPlus_IBM_VGA_8x16.ttf"
# https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont_all-18.0.01.hex.gz (gunzip it)
# https://ftp.gnu.org/gnu/unifont/unifont-18.0.01/unifont-combining-18.0.01.txt
python3 gen.py --vga PxPlus_IBM_VGA_8x16.ttf \
    --unifont unifont_all-18.0.01.hex \
    --combining unifont-combining-18.0.01.txt
```

`gen.py` needs fontTools and Pillow. It writes the three data files next to itself;
`cargo test -p hecate render::fonts` checks that the index and the picture agree.
