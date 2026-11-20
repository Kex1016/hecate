//! The bitmap faces of the window label: IBM VGA 8x16 first, then GNU Unifont for what it
//! lacks. Both sit on the same 8x16 grid (Unifont's wide glyphs take two cells), so a line can
//! mix them. `README.md` here has the sources and licences, `gen.py` rebuilds the data.

use std::sync::LazyLock;

static VGA: &[u8] = include_bytes!("vga8x16.bin");
static UNIFONT_INDEX: &[u8] = include_bytes!("unifont.idx");
static UNIFONT_CELLS: &[u8] = include_bytes!("unifont.png");

/// Glyph height in font pixels. A cell is 8 wide.
pub const HEIGHT: usize = 16;

/// One glyph: `width` is 8 or 16, each row has its leftmost pixel in bit 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub rows: [u16; HEIGHT],
    pub width: u8,
    /// A combining mark: drawn over what came before at this offset from the pen, which
    /// stays where it is.
    pub overlay: Option<i8>,
}

impl Glyph {
    fn blank(width: u8) -> Glyph {
        Glyph {
            rows: [0; HEIGHT],
            width,
            overlay: None,
        }
    }
}

/// What a label can do with a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    Glyph(Glyph),
    /// Takes no room and draws nothing (joiners, variation selectors, direction marks).
    Skip,
    /// Neither face has it.
    Missing,
}

pub fn lookup(c: char) -> Lookup {
    let cp = c as u32;
    match cp {
        0x09 | 0x0A | 0x0D | 0x20 => return Lookup::Glyph(Glyph::blank(8)),
        0x00AD
        | 0x034F
        | 0x061C
        | 0x180B..=0x180F
        | 0x200B..=0x200F
        | 0x202A..=0x202E
        | 0x2060..=0x206F
        | 0xFE00..=0xFE0F
        | 0xFEFF
        | 0x1F3FB..=0x1F3FF
        | 0xE0000..=0xE0FFF => return Lookup::Skip,
        0x2000..=0x200A | 0x202F | 0x205F => return Lookup::Glyph(Glyph::blank(8)),
        0x3000 => return Lookup::Glyph(Glyph::blank(16)),
        _ => {}
    }
    if let Some(g) = vga(cp) {
        return Lookup::Glyph(g);
    }
    match UNIFONT.as_ref().and_then(|u| u.glyph(cp)) {
        Some(g) => Lookup::Glyph(g),
        None => Lookup::Missing,
    }
}

/// `vga8x16.bin`: records of a little-endian code point and 16 rows, sorted by code point.
fn vga(cp: u32) -> Option<Glyph> {
    const RECORD: usize = 4 + HEIGHT;
    let count = VGA.len() / RECORD;
    let at = |i: usize| u32::from_le_bytes(VGA[i * RECORD..i * RECORD + 4].try_into().unwrap());
    let (mut lo, mut hi) = (0, count);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if at(mid) < cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == count || at(lo) != cp {
        return None;
    }
    let bits = &VGA[lo * RECORD + 4..(lo + 1) * RECORD];
    let mut rows = [0u16; HEIGHT];
    for (row, b) in rows.iter_mut().zip(bits) {
        *row = (*b as u16) << 8;
    }
    Some(Glyph {
        rows,
        width: 8,
        overlay: None,
    })
}

const WIDE: u8 = 1;
const COMBINING: u8 = 2;
/// Unifont's baseline is this many rows below the VGA face's.
const BASELINE_SHIFT: usize = 2;

/// A run of consecutive code points of one kind, and the cell its first glyph starts at.
struct Run {
    start: u32,
    len: u32,
    flags: u8,
    offset: i8,
    cell: usize,
}

struct Unifont {
    runs: Vec<Run>,
    /// 16 bytes per 8x16 cell, in the order of the runs.
    cells: Vec<u8>,
}

/// Decoded on first use: some tens of milliseconds, once, and only when a title has a
/// character the VGA face lacks.
static UNIFONT: LazyLock<Option<Unifont>> = LazyLock::new(|| {
    Unifont::load()
        .map_err(|e| tracing::warn!("error decoding the fallback font: {e:#}"))
        .ok()
});

impl Unifont {
    fn load() -> anyhow::Result<Unifont> {
        let mut runs = Vec::with_capacity(UNIFONT_INDEX.len() / 8);
        let mut cell = 0;
        for r in UNIFONT_INDEX.chunks_exact(8) {
            let run = Run {
                start: u32::from_le_bytes(r[0..4].try_into().unwrap()),
                len: u16::from_le_bytes(r[4..6].try_into().unwrap()) as u32,
                flags: r[6],
                offset: r[7] as i8,
                cell,
            };
            cell += run.len as usize * if run.flags & WIDE != 0 { 2 } else { 1 };
            runs.push(run);
        }

        // A 1-bit picture, 256 cells to a row: one byte is one row of one cell.
        let mut decoder = png::Decoder::new(UNIFONT_CELLS);
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info()?;
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels)?;
        anyhow::ensure!(
            info.color_type == png::ColorType::Grayscale
                && info.bit_depth == png::BitDepth::One
                && info.width == 2048
                && info.line_size == 256,
            "unexpected picture format"
        );
        let lines = info.height as usize / HEIGHT;
        anyhow::ensure!(
            cell <= lines * 256,
            "the index wants more cells than the picture has"
        );
        let mut cells = vec![0; cell * HEIGHT];
        for (i, out) in cells.chunks_exact_mut(HEIGHT).enumerate() {
            let (x, y) = (i % 256, i / 256 * HEIGHT);
            for (r, b) in out.iter_mut().enumerate() {
                *b = pixels[(y + r) * 256 + x];
            }
        }
        Ok(Unifont { runs, cells })
    }

    fn glyph(&self, cp: u32) -> Option<Glyph> {
        let i = self.runs.partition_point(|r| r.start <= cp).checked_sub(1)?;
        let run = &self.runs[i];
        if cp - run.start >= run.len {
            return None;
        }
        let wide = run.flags & WIDE != 0;
        let combining = run.flags & COMBINING != 0;
        let cell = run.cell + (cp - run.start) as usize * if wide { 2 } else { 1 };
        let left = &self.cells[cell * HEIGHT..(cell + 1) * HEIGHT];
        let mut rows = [0u16; HEIGHT];
        for (r, row) in rows.iter_mut().enumerate() {
            *row = (left[r] as u16) << 8;
            if wide {
                *row |= self.cells[(cell + 1) * HEIGHT + r] as u16;
            }
        }
        // Letters move up onto the VGA baseline when they have the room above. Wide glyphs
        // (CJK, emoji) fill their box and stay, as do marks, which go by their base.
        if !wide && !combining {
            let free = rows.iter().take(BASELINE_SHIFT).take_while(|r| **r == 0).count();
            rows.rotate_left(free);
        }
        Some(Glyph {
            rows,
            width: if wide { 16 } else { 8 },
            overlay: combining.then_some(run.offset),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(c: char) -> Glyph {
        match lookup(c) {
            Lookup::Glyph(g) => g,
            other => panic!("{c:?}: {other:?}"),
        }
    }

    #[test]
    fn vga_has_latin_and_box_drawing() {
        // "A" in the VGA face: the top of the letter is a 2-pixel point on row 2.
        let a = glyph('A');
        assert_eq!((a.width, a.overlay), (8, None));
        assert_eq!(a.rows[0], 0);
        assert_eq!(a.rows[2], 0b0001_0000 << 8);
        assert_eq!(a.rows[3], 0b0011_1000 << 8);
        // Full block fills the cell.
        assert!(glyph('█').rows.iter().all(|r| *r == 0xFF00));
        for c in "kitty: ~/src/wl-hex őű Ж λ €…".chars() {
            assert_eq!(glyph(c).width, 8, "{c:?} comes from the VGA face");
            assert!(vga(c as u32).is_some() || c == ' ', "{c:?}");
        }
    }

    #[test]
    fn unifont_fills_in() {
        for c in "日本語한국어😀".chars() {
            assert!(vga(c as u32).is_none());
            let g = glyph(c);
            assert_eq!(g.width, 16, "{c:?}");
            assert!(g.rows.iter().any(|r| *r != 0), "{c:?} has pixels");
            assert!(g.rows.iter().any(|r| r & 0xFF != 0), "{c:?} uses its right half");
        }
        // A narrow letter the VGA face lacks moves up onto the VGA baseline: it ends on
        // row 11, as "e" does there.
        let a = glyph('ա');
        assert_eq!(a.width, 8);
        assert!(a.rows[11] != 0 && a.rows[12] == 0);
        assert!(glyph('e').rows[11] != 0 && glyph('e').rows[12] == 0);
        // One with accents stacked up to the second row only has one row to move.
        let e = glyph('ế');
        assert!(e.rows[0] != 0 && e.rows[12] != 0 && e.rows[13] == 0);
        // Combining acute: a mark over the cell before it.
        assert_eq!(glyph('\u{301}').overlay, Some(-8));
    }

    #[test]
    fn unrenderable_and_invisible_characters() {
        // Right-to-left and shaped scripts are left out on purpose.
        for c in "שلاमက".chars() {
            assert_eq!(lookup(c), Lookup::Missing, "{c:?}");
        }
        assert_eq!(lookup('\u{E000}'), Lookup::Missing, "private use");
        for c in ['\u{200D}', '\u{FE0F}', '\u{200E}', '\u{AD}'] {
            assert_eq!(lookup(c), Lookup::Skip, "{c:?}");
        }
        assert_eq!(glyph('\u{3000}'), Glyph::blank(16));
        assert_eq!(glyph('\t'), Glyph::blank(8));
    }

    #[test]
    fn index_and_picture_agree() {
        let u = UNIFONT.as_ref().expect("the fallback font decodes");
        let last = u.runs.last().unwrap();
        let cells = last.cell + last.len as usize * if last.flags & WIDE != 0 { 2 } else { 1 };
        assert_eq!(u.cells.len(), cells * HEIGHT);
        assert!(u.runs.windows(2).all(|w| w[0].start + w[0].len <= w[1].start));
    }
}
