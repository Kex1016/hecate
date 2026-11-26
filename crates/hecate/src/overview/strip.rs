//! The workspace strip above the overview's grid: one live thumbnail per workspace of the
//! output, to switch to or drop a window on.

use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::render::border::Extents;

/// Share of the strip's area height its row takes.
pub const HEIGHT: f64 = 0.12;

/// One workspace's thumbnail.
#[derive(Debug, Clone, PartialEq)]
pub struct Thumb {
    pub workspace: u64,
    /// 1-based position among the output's workspaces.
    pub index: u32,
    pub frame: Rectangle<i32, Logical>,
    pub content: Rectangle<i32, Logical>,
    /// Thumbnail pixels per output pixel.
    pub scale: f64,
}

/// A window being dragged out of its slot.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    pub window: Window,
    pub start: Point<f64, Logical>,
    pub pos: Point<f64, Logical>,
    /// Moved far enough to be a drag, not a click.
    pub active: bool,
}

/// How far a press may move and still be a click, in logical pixels.
pub const DRAG_THRESHOLD: f64 = 8.0;

/// The strip's row height for an area.
pub fn height(area: Rectangle<i32, Logical>) -> i32 {
    (area.size.h as f64 * HEIGHT).round() as i32
}

/// Lays `count` thumbnails out along the top of `area`, each with the output's aspect
/// ratio, centred, shrunk to fit when there are many. `workspace` and `index` are left
/// for the caller to fill in.
pub fn strip(
    area: Rectangle<i32, Logical>,
    output: Size<i32, Logical>,
    count: usize,
    gap: i32,
    extents: Extents,
) -> Vec<Thumb> {
    if count == 0 {
        return Vec::new();
    }
    let row = height(area);
    let aspect = output.w.max(1) as f64 / output.h.max(1) as f64;
    let (ex, ey) = (extents.left + extents.right, extents.top + extents.bottom);
    let mut content_h = (row - ey).max(1);
    let mut content_w = ((content_h as f64 * aspect).round() as i32).max(1);
    let n = count as i32;
    if n * (content_w + ex) + (n - 1) * gap > area.size.w {
        let frame_w = (area.size.w - (n - 1) * gap) / n;
        content_w = (frame_w - ex).max(1);
        content_h = ((content_w as f64 / aspect).floor() as i32).max(1);
    }
    let (frame_w, frame_h) = (content_w + ex, content_h + ey);
    let total = n * frame_w + (n - 1) * gap;
    let x0 = area.loc.x + (area.size.w - total) / 2;
    let y = area.loc.y + (row - frame_h) / 2;
    (0..n)
        .map(|i| {
            let content = Rectangle::new(
                (x0 + i * (frame_w + gap) + extents.left, y + extents.top).into(),
                (content_w, content_h).into(),
            );
            Thumb {
                workspace: 0,
                index: i as u32 + 1,
                frame: extents.around(content),
                content,
                scale: content_w as f64 / output.w.max(1) as f64,
            }
        })
        .collect()
}
