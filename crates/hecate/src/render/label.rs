//! Small text labels (`identify`), drawn with an 8×8 bitmap font.

use std::cell::RefCell;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::Transform;

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
}
