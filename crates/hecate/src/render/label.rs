//! Small text labels: `identify`'s, drawn with an 8×8 bitmap font, and the window frame's
//! notch label ("app: title"), drawn with the 8x16 faces in `fonts`.

use std::cell::RefCell;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::Transform;

use super::fonts::{self, Glyph, Lookup};
use crate::config::Color;

/// Glyph pixels per font pixel.
const ZOOM: usize = 2;
const PAD: usize = 6;

/// Pixels of `text` (unknown characters as `?`): ARGB8888 bytes, width, height.
pub fn rasterize(text: &str, fg: Color, bg: Color) -> (Vec<u8>, usize, usize) {
    let chars: Vec<usize> = text
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c as usize
            } else {
                b'?' as usize
            }
        })
        .collect();
    let glyph = 8 * ZOOM;
    let w = chars.len() * glyph + 2 * PAD;
    let h = glyph + 2 * PAD;
    let px = |c: Color| {
        // Premultiplied ARGB8888, little endian: B, G, R, A.
        let [r, g, b, a] = c
            .premultiplied()
            .map(|v| (v * 255.).round().clamp(0., 255.) as u8);
        [b, g, r, a]
    };
    let (fg, bg) = (px(fg), px(bg));
    let mut buf = Vec::with_capacity(w * h * 4);
    for _ in 0..w * h {
        buf.extend_from_slice(&bg);
    }
    for (i, &c) in chars.iter().enumerate() {
        let rows = font8x8::legacy::BASIC_LEGACY[c];
        for (gy, row) in rows.iter().enumerate() {
            for gx in 0..8 {
                // Bit 0 is the leftmost pixel.
                if row & (1 << gx) == 0 {
                    continue;
                }
                for dy in 0..ZOOM {
                    for dx in 0..ZOOM {
                        let x = PAD + i * glyph + gx * ZOOM + dx;
                        let y = PAD + gy * ZOOM + dy;
                        let o = (y * w + x) * 4;
                        buf[o..o + 4].copy_from_slice(&fg);
                    }
                }
            }
        }
    }
    (buf, w, h)
}

/// A label's buffer, kept per window while its text stays the same (stable damage).
#[derive(Default)]
pub struct LabelCache(RefCell<Option<(String, MemoryRenderBuffer)>>);

impl LabelCache {
    pub fn get(&self, text: &str, fg: Color, bg: Color) -> MemoryRenderBuffer {
        let mut cached = self.0.borrow_mut();
        if let Some((t, buf)) = cached.as_ref()
            && t == text
        {
            return buf.clone();
        }
        let (pixels, w, h) = rasterize(text, fg, bg);
        let buf = MemoryRenderBuffer::from_slice(
            &pixels,
            Fourcc::Argb8888,
            (w as i32, h as i32),
            1,
            Transform::Normal,
            None,
        );
        *cached = Some((text.to_string(), buf.clone()));
        buf
    }
}

/// Room left and right of a notch label's text, in font pixels (one cell: CSS `1ch`).
pub const NOTCH_PAD_X: usize = 8;
/// Room above and below it.
pub const NOTCH_PAD_Y: usize = 1;
/// Height of a notch label in logical pixels: a 16 px line at line-height 1.15.
pub const NOTCH_HEIGHT: i32 = (fonts::HEIGHT + 2 * NOTCH_PAD_Y) as i32;

/// A line of glyphs on the 8x16 grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Glyphs and where their left edge goes, in font pixels.
    glyphs: Vec<(usize, Glyph)>,
    /// Width in font pixels.
    pub width: usize,
}

/// Lay `text` out, cut to `max_width` font pixels with "…". None when a character has no
/// glyph in either face, or not even "…" fits.
pub fn layout(text: &str, max_width: usize) -> Option<Line> {
    let mut glyphs = Vec::new();
    let mut pen: usize = 0;
    for c in text.chars() {
        let g = match fonts::lookup(c) {
            Lookup::Glyph(g) => g,
            Lookup::Skip => continue,
            Lookup::Missing => return None,
        };
        match g.overlay {
            // A mark with nothing before it to sit on is dropped.
            Some(offset) => {
                if let Some(x) = pen.checked_add_signed(offset as isize) {
                    glyphs.push((x, g));
                }
            }
            None => {
                glyphs.push((pen, g));
                pen += g.width as usize;
            }
        }
    }
    if pen <= max_width {
        return Some(Line { glyphs, width: pen });
    }
    let Lookup::Glyph(ellipsis) = fonts::lookup('…') else {
        return None;
    };
    let room = max_width.checked_sub(ellipsis.width as usize)?;
    glyphs.retain(|(x, g)| x + g.width as usize <= room);
    let end = glyphs
        .iter()
        .filter(|(_, g)| g.overlay.is_none())
        .map(|(x, g)| x + g.width as usize)
        .max()
        .unwrap_or(0);
    glyphs.push((end, ellipsis));
    Some(Line {
        glyphs,
        width: end + ellipsis.width as usize,
    })
}

/// The short name a window's label opens with: the last part of a reverse-DNS app id
/// (`org.gnome.Nautilus` is `nautilus`), in lower case.
pub fn app_name(app_id: &str) -> String {
    let id = app_id.strip_suffix(".desktop").unwrap_or(app_id);
    let parts: Vec<&str> = id.split('.').collect();
    let name = match parts.last() {
        Some(last) if parts.len() >= 3 && last.starts_with(char::is_alphabetic) => last,
        _ => id,
    };
    clean(name).to_lowercase()
}

/// One line: control characters and runs of white space become single spaces.
fn clean(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A window's notch label, "app: title", at most `max_width` font pixels wide. A title with
/// a character neither face can draw (or that needs shaping, see `fonts`) leaves the app
/// name alone; an app name like that is spelled with `?`.
pub fn window_line(app_id: &str, title: &str, max_width: usize) -> Option<Line> {
    let app = app_name(app_id);
    let title = clean(title);
    let both = if app.is_empty() || title.is_empty() || title.to_lowercase() == app {
        None
    } else {
        Some(format!("{app}: {title}"))
    };
    let alone = if app.is_empty() { title } else { app };
    if alone.is_empty() {
        return None;
    }
    if let Some(line) = both.and_then(|text| layout(&text, max_width)) {
        return Some(line);
    }
    layout(&alone, max_width).or_else(|| {
        let ascii: String = alone
            .chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect();
        layout(&ascii, max_width)
    })
}

/// Pixels of a notch label: the line on `bg` with its padding, `zoom` buffer pixels per
/// font pixel. ARGB8888 bytes, width, height.
pub fn rasterize_line(line: &Line, fg: Color, bg: Color, zoom: usize) -> (Vec<u8>, usize, usize) {
    let zoom = zoom.max(1);
    let w = (line.width + 2 * NOTCH_PAD_X) * zoom;
    let h = (fonts::HEIGHT + 2 * NOTCH_PAD_Y) * zoom;
    let px = |c: Color| {
        let [r, g, b, a] = c
            .premultiplied()
            .map(|v| (v * 255.).round().clamp(0., 255.) as u8);
        [b, g, r, a]
    };
    let (fg, bg) = (px(fg), px(bg));
    let mut buf = Vec::with_capacity(w * h * 4);
    for _ in 0..w * h {
        buf.extend_from_slice(&bg);
    }
    for (gx, glyph) in &line.glyphs {
        for (gy, row) in glyph.rows.iter().enumerate() {
            for bit in 0..glyph.width as usize {
                if row & (0x8000 >> bit) == 0 {
                    continue;
                }
                let x0 = (NOTCH_PAD_X + gx + bit) * zoom;
                let y0 = (NOTCH_PAD_Y + gy) * zoom;
                for y in y0..y0 + zoom {
                    for x in x0..x0 + zoom {
                        let o = (y * w + x) * 4;
                        buf[o..o + 4].copy_from_slice(&fg);
                    }
                }
            }
        }
    }
    (buf, w, h)
}

/// What a window's notch label was drawn from.
#[derive(Debug, Clone, PartialEq)]
struct NotchKey {
    app_id: String,
    title: String,
    max_width: usize,
    fg: Color,
    bg: Color,
    zoom: usize,
}

/// A label's buffer and its width in logical pixels.
type Notch = (MemoryRenderBuffer, i32);

/// A window's notch label, kept while nothing it is drawn from changes (stable damage).
#[derive(Default)]
pub struct NotchCache(RefCell<Option<(NotchKey, Option<Notch>)>>);

impl NotchCache {
    /// The label's buffer and its width in logical pixels, or None when there is nothing
    /// to show or no room for it. `max_width` is in logical pixels, padding included.
    pub fn get(
        &self,
        app_id: &str,
        title: &str,
        max_width: i32,
        fg: Color,
        bg: Color,
        zoom: usize,
    ) -> Option<Notch> {
        let max_width = (max_width.max(0) as usize).saturating_sub(2 * NOTCH_PAD_X);
        let mut cached = self.0.borrow_mut();
        if let Some((key, label)) = cached.as_ref()
            && key.app_id == app_id
            && key.title == title
            && (key.max_width, key.fg, key.bg, key.zoom) == (max_width, fg, bg, zoom)
        {
            return label.clone();
        }
        let label = window_line(app_id, title, max_width).map(|line| {
            let (pixels, w, h) = rasterize_line(&line, fg, bg, zoom);
            let buf = MemoryRenderBuffer::from_slice(
                &pixels,
                Fourcc::Argb8888,
                (w as i32, h as i32),
                zoom as i32,
                Transform::Normal,
                None,
            );
            (buf, (line.width + 2 * NOTCH_PAD_X) as i32)
        });
        let key = NotchKey {
            app_id: app_id.to_string(),
            title: title.to_string(),
            max_width,
            fg,
            bg,
            zoom,
        };
        *cached = Some((key, label.clone()));
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_glyphs_with_padding() {
        let white = Color::rgb(255, 255, 255);
        let black = Color::rgb(0, 0, 0);
        let (buf, w, h) = rasterize("I", white, black);
        assert_eq!((w, h), (16 + 12, 16 + 12));
        let at = |x: usize, y: usize| buf[(y * w + x) * 4];
        assert_eq!(at(0, 0), 0, "padding is background");
        // "I" in font8x8 has its stem in the middle columns.
        let stem = (0..w).filter(|&x| at(x, PAD + 8) == 255).count();
        assert!(stem > 0 && stem < 16, "a narrow stem: {stem} px");
        let (_, w2, _) = rasterize("é?", white, black);
        assert_eq!(w2, 2 * 16 + 12, "non-ASCII is one replacement glyph");
    }

    #[test]
    fn app_names() {
        assert_eq!(app_name("kitty"), "kitty");
        assert_eq!(app_name("org.gnome.Nautilus"), "nautilus");
        assert_eq!(app_name("com.mitchellh.ghostty"), "ghostty");
        assert_eq!(app_name("gimp-2.10"), "gimp-2.10");
        assert_eq!(app_name("Steam"), "steam");
        assert_eq!(app_name("org.kde.dolphin.desktop"), "dolphin");
        assert_eq!(app_name(""), "");
    }

    #[test]
    fn window_lines() {
        let width = |app: &str, title: &str| window_line(app, title, 4000).map(|l| l.width);
        // The canvas's own: 19 cells.
        assert_eq!(
            width("kitty", "~/src/wl-hex"),
            Some("kitty: ~/src/wl-hex".len() * 8)
        );
        assert_eq!(
            width("org.gnome.Nautilus", "Nautilus"),
            Some(8 * 8),
            "no 'nautilus: Nautilus'"
        );
        assert_eq!(width("kitty", ""), Some(5 * 8));
        assert_eq!(width("", "untitled"), Some(8 * 8));
        assert_eq!(width("", " \n "), None);
        // Wide glyphs from the fallback face take two cells; the VGA face has ő itself.
        assert_eq!(width("foot", "日本語"), Some((6 + 6) * 8));
        assert_eq!(width("foot", "őrület"), Some((6 + 6) * 8));
        // A zero-width joiner sequence and a variation selector take no room.
        assert_eq!(width("foot", "❤\u{FE0F}"), Some((6 + 2) * 8));
        // A combining mark sits on the letter before it.
        assert_eq!(width("foot", "e\u{301}"), Some((6 + 1) * 8));
        // Hebrew would come out backwards, so the title is dropped, not mangled.
        assert_eq!(width("foot", "שלום"), Some(4 * 8));
        assert_eq!(
            width("שלום", "שלום"),
            Some(4 * 8),
            "an app name it can't draw is spelled with ?"
        );
    }

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        let line = window_line("kitty", "a very long title indeed", 12 * 8).unwrap();
        assert_eq!(line.width, 12 * 8);
        let (x, last) = *line.glyphs.last().unwrap();
        assert_eq!(x, 11 * 8);
        assert_eq!(Lookup::Glyph(last), fonts::lookup('…'));
        // A wide glyph that would straddle the cut goes whole.
        let line = window_line("", "日本語日本語", 5 * 8).unwrap();
        assert_eq!(line.width, 5 * 8);
        assert_eq!(line.glyphs.len(), 3);
        assert_eq!(window_line("kitty", "x", 0), None);
    }

    #[test]
    fn notch_pixels() {
        let fg = Color::rgb(0xe0, 0xde, 0xf4);
        let bg = Color::rgb(0x1f, 0x1d, 0x2e);
        let line = layout("I", 100).unwrap();
        let (buf, w, h) = rasterize_line(&line, fg, bg, 1);
        assert_eq!((w, h), (8 + 16, 18));
        let at = |x: usize, y: usize| {
            [
                buf[(y * w + x) * 4 + 2],
                buf[(y * w + x) * 4 + 1],
                buf[(y * w + x) * 4],
            ]
        };
        assert_eq!(at(0, 0), [0x1f, 0x1d, 0x2e], "padding is the band colour");
        // The VGA "I": a serif on row 2, two pixels in from each side of its cell.
        assert_eq!(at(NOTCH_PAD_X + 2, NOTCH_PAD_Y + 2), [0xe0, 0xde, 0xf4]);
        assert_eq!(at(NOTCH_PAD_X, NOTCH_PAD_Y + 2), [0x1f, 0x1d, 0x2e]);
        assert_eq!(at(NOTCH_PAD_X + 2, NOTCH_PAD_Y + 1), [0x1f, 0x1d, 0x2e]);
        // Twice the size is the same picture with every pixel doubled.
        let (buf2, w2, h2) = rasterize_line(&line, fg, bg, 2);
        assert_eq!((w2, h2), (2 * w, 2 * h));
        for (x, y) in [
            (0, 0),
            (NOTCH_PAD_X + 2, NOTCH_PAD_Y + 2),
            (NOTCH_PAD_X + 3, NOTCH_PAD_Y + 5),
        ] {
            let o = (2 * y * w2 + 2 * x) * 4;
            assert_eq!(buf2[o..o + 4], buf[(y * w + x) * 4..(y * w + x) * 4 + 4]);
            assert_eq!(buf2[o + 4..o + 8], buf2[o..o + 4]);
        }
    }
}
