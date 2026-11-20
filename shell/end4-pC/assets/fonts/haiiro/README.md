# haiiro fonts

The four faces the shell is set in. `Appearance.qml` loads them from here, `nix/fonts.nix`
installs them system-wide, and `web/` holds `.woff2` copies for web pages.

| file | family | role | upstream | licence |
| --- | --- | --- | --- | --- |
| `IBMVGA8x16-Regular.ttf` | IBM VGA 8x16 | mono, the face everything is set in | PxPlus IBM VGA 8x16, The Ultimate Oldschool PC Font Pack v2.2 | CC BY-SA 4.0 |
| `IBMVGA8x16-Bold.ttf` | IBM VGA 8x16 (weight 700) | bold: the double-width 2x face | PxPlus IBM VGA 8x16-2x, same pack | CC BY-SA 4.0 |
| `Jacquard24-Regular.ttf` | Jacquard 24 | display titles and the desktop clock | Jacquard 24 v1.002, Google Fonts | OFL 1.1 |
| `Unifont.otf` | Unifont | fallback for glyphs the others lack | GNU Unifont 18.0.01 | OFL 1.1, or GPL 2+ with the font embedding exception |
| `UnifontUpper.otf` | Unifont Upper | fallback beyond the Basic Multilingual Plane | GNU Unifont 18.0.01 | OFL 1.1, or GPL 2+ with the font embedding exception |

The licence texts are in `licenses/`.

## Stacks

- mono: IBM VGA 8x16, Unifont, Unifont Upper
- display: Jacquard 24, IBM VGA 8x16, Unifont

Unifont is drawn on the same 8x16 grid as the VGA face, so a fallback glyph keeps the cell.

QML takes one family per font, so the shell sets `font.family` to the first face and
`45-haiiro.conf` tells fontconfig where missing glyphs come from. The file has to be installed
for that (`nix/fonts.nix` puts it in `etc/fonts/conf.d`); without it a missing glyph comes from
whatever font the system prefers.

Bold is never synthesised. The 2x face is the family's weight 700, so `font.bold` (QML) or
`font-weight: 700` (CSS) switches to it. It is twice as wide as the regular face: a terminal
that must keep its columns should use the regular face for bold text too.

## Sources

Fetched on 2026-11-19. `build-fonts.py` checks these sums before it builds.

| download | sha256 |
| --- | --- |
| <https://int10h.org/oldschool-pc-fonts/download/oldschool_pc_font_pack_v2.2_FULL.zip> | `21b3c0a3770ef0afc46564760613d7b078f4fcc9ed93db4b829a440b68822e08` |
| <https://raw.githubusercontent.com/google/fonts/main/ofl/jacquard24/Jacquard24-Regular.ttf> | `8e5b1bdb8b6087d6175f1d9edd2634e38b10fb109c0955c24ad55272a866ed49` |
| <https://unifoundry.com/pub/unifont/unifont-18.0.01/font-builds/unifont-18.0.01.otf> | `88d0a14d4aa9a96419720b39ba5da921a59560d76d4ec7d523acc06ed85cd3c2` |
| <https://unifoundry.com/pub/unifont/unifont-18.0.01/font-builds/unifont_upper-18.0.01.otf> | `472201e45a050bff4c7658208b8c73e34ebce3764b287a5a46c7774038ef9391` |

From the font pack: `ttf - Px (pixel outline)/PxPlus_IBM_VGA_8x16.ttf` and
`PxPlus_IBM_VGA_8x16-2x.ttf`. PxPlus is the pack's extended set (Latin, Greek, Cyrillic, box
drawing and more on top of code page 437), with the same glyphs and metrics as its Web437 files.

## What was changed

- **IBM VGA 8x16**: the two PxPlus files are registered as one family, "IBM VGA 8x16", with the
  2x face as its bold. Only the `name`, `OS/2` and `head` tables changed (family, style and full
  names, weight class, the bold bits), and FontForge's `FFTM` timestamp table was dropped. The
  glyphs, metrics and character map are untouched. These two files are adaptations of
  The Ultimate Oldschool PC Font Pack by VileR (<https://int10h.org/oldschool-pc-fonts/>), and are
  shared under the same licence, CC BY-SA 4.0.
- **Jacquard 24, Unifont, Unifont Upper**: byte-for-byte copies of the downloads, renamed.
- **`web/*.woff2`**: the same five files in WOFF2, nothing else changed.

To rebuild everything from the downloads:

```sh
pip install fonttools brotli
python3 build-fonts.py <directory with the four downloads>
```

## On a web page

```css
@font-face { font-family: "IBM VGA 8x16"; font-weight: 400; src: url("IBMVGA8x16-Regular.woff2") format("woff2"); }
@font-face { font-family: "IBM VGA 8x16"; font-weight: 700; src: url("IBMVGA8x16-Bold.woff2") format("woff2"); }
@font-face { font-family: "Jacquard 24"; src: url("Jacquard24-Regular.woff2") format("woff2"); }
@font-face { font-family: "Unifont"; src: url("Unifont.woff2") format("woff2"); }
@font-face { font-family: "Unifont Upper"; src: url("UnifontUpper.woff2") format("woff2"); }

:root {
  --font-mono: "IBM VGA 8x16", "Unifont", "Unifont Upper", monospace;
  --font-display: "Jacquard 24", "IBM VGA 8x16", "Unifont", serif;
}
```

The two Unifont files are large (0.9 MB and 0.6 MB as WOFF2). A browser only fetches a face
when a page needs a glyph from it.

## Credits

- The Ultimate Oldschool PC Font Pack v2.2, (c) 2016-2020 VileR, <https://int10h.org/oldschool-pc-fonts/>.
- Jacquard 24, (c) 2023 The Soft Type Project Authors, <https://github.com/scfried/soft-type-jacquard>.
- GNU Unifont, by Roman Czyborra, Paul Hardy and contributors, <https://unifoundry.com/unifont/>.
