//! Where each window of the overview goes: a pure grid layout.

use smithay::utils::{Logical, Rectangle};

use crate::config::action::Direction;
use crate::render::border::Extents;

/// A window as the overview finds it: its content rect now, and its frame extents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub rect: Rectangle<i32, Logical>,
    pub extents: Extents,
}

/// Where one window goes in the overview.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub frame: Rectangle<i32, Logical>,
    pub content: Rectangle<i32, Logical>,
    pub scale: f64,
}

/// The scale that fits `item` with its frame into a cell of `cell` size, never above 1.
fn fit(item: &Item, cell: (i32, i32)) -> f64 {
    let ext = item.extents;
    let w = f64::from(cell.0 - ext.left - ext.right) / f64::from(item.rect.size.w.max(1));
    let h = f64::from(cell.1 - ext.top - ext.bottom) / f64::from(item.rect.size.h.max(1));
    w.min(h).clamp(0.0, 1.0)
}

/// The cell size for `n` items in `columns` columns, or `None` when it has no room.
fn cell_size(area: Rectangle<i32, Logical>, n: usize, columns: usize, gap: i32) -> Option<(i32, i32)> {
    let c = columns as i32;
    let r = n.div_ceil(columns) as i32;
    let w = (area.size.w - gap * (c + 1)) / c;
    let h = (area.size.h - gap * (r + 1)) / r;
    (w > 0 && h > 0).then_some((w, h))
}

/// Lays `items` out in a grid inside `area`. The result has the same length and order
/// as `items`; which item gets which cell follows the items' reading order, so windows
/// land near where they were.
pub fn grid(area: Rectangle<i32, Logical>, items: &[Item], gap: i32) -> Vec<Slot> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }
    // The column count that shows the most window area; ties go to the squarest grid
    // (columns nearest √n), then to fewer columns.
    let square = (n as f64).sqrt().ceil();
    let mut best: Option<(f64, usize, (i32, i32))> = None;
    for c in 1..=n {
        let Some(cell) = cell_size(area, n, c, gap) else {
            continue;
        };
        let shown: f64 = items
            .iter()
            .map(|it| {
                let s = fit(it, cell);
                f64::from(it.rect.size.w) * s * f64::from(it.rect.size.h) * s
            })
            .sum();
        let better = best.is_none_or(|(b, bc, _)| {
            let tie = (shown - b).abs() <= b.abs() * 1e-9;
            if tie {
                (c as f64 - square).abs() < (bc as f64 - square).abs()
            } else {
                shown > b
            }
        });
        if better {
            best = Some((shown, c, cell));
        }
    }
    let (columns, cell) = match best {
        Some((_, c, cell)) => (c, cell),
        // No column count leaves room: stack everything in one tiny cell per item.
        None => (n, (1, 1)),
    };
    let c = columns as i32;

    let rects: Vec<_> = items.iter().map(|it| it.rect).collect();
    let order = reading_order(&rects, columns);
    let mut slots = vec![
        Slot {
            frame: Rectangle::default(),
            content: Rectangle::default(),
            scale: 1.0
        };
        n
    ];
    for (k, &i) in order.iter().enumerate() {
        let item = &items[i];
        let (row, col) = ((k / columns) as i32, (k % columns) as i32);
        let in_row = (n - (row as usize) * columns).min(columns) as i32;
        let row_x = area.loc.x + gap + (c - in_row) * (cell.0 + gap) / 2;
        let cell_loc = (
            row_x + col * (cell.0 + gap),
            area.loc.y + gap + row * (cell.1 + gap),
        );

        let scale = fit(item, cell);
        let size = (
            ((f64::from(item.rect.size.w) * scale).floor() as i32).max(1),
            ((f64::from(item.rect.size.h) * scale).floor() as i32).max(1),
        );
        let ext = item.extents;
        let frame_size = (size.0 + ext.left + ext.right, size.1 + ext.top + ext.bottom);
        let frame_loc = (
            cell_loc.0 + (cell.0 - frame_size.0) / 2,
            cell_loc.1 + (cell.1 - frame_size.1) / 2,
        );
        let content = Rectangle::new(
            (frame_loc.0 + ext.left, frame_loc.1 + ext.top).into(),
            size.into(),
        );
        slots[i] = Slot {
            frame: ext.around(content),
            content,
            scale,
        };
    }
    slots
}

fn centre(rect: Rectangle<i32, Logical>) -> (f64, f64) {
    (
        f64::from(rect.loc.x) + f64::from(rect.size.w) / 2.0,
        f64::from(rect.loc.y) + f64::from(rect.size.h) / 2.0,
    )
}

/// Indices of `rects` in reading order: by centre y, in rows of `columns`, each row by
/// centre x.
pub fn reading_order(rects: &[Rectangle<i32, Logical>], columns: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|&a, &b| centre(rects[a]).1.total_cmp(&centre(rects[b]).1));
    for row in order.chunks_mut(columns.max(1)) {
        row.sort_by(|&a, &b| centre(rects[a]).0.total_cmp(&centre(rects[b]).0));
    }
    order
}

/// The eligible slot nearest to `from` in direction `dir`, by distance along `dir` plus
/// twice the distance across it.
pub fn neighbour(slots: &[Slot], from: usize, dir: Direction, eligible: &[bool]) -> Option<usize> {
    let (fx, fy) = centre(slots.get(from)?.frame);
    slots
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != from && eligible.get(i).copied().unwrap_or(false))
        .filter_map(|(i, s)| {
            let (x, y) = centre(s.frame);
            let (dx, dy) = (x - fx, y - fy);
            let (primary, cross) = match dir {
                Direction::Right => (dx, dy.abs()),
                Direction::Left => (-dx, dy.abs()),
                Direction::Down => (dy, dx.abs()),
                Direction::Up => (-dy, dx.abs()),
            };
            (primary > 0.0).then_some((i, primary + 2.0 * cross))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXT: Extents = Extents {
        top: 13,
        right: 5,
        bottom: 5,
        left: 5,
    };
    fn at(x: i32, y: i32, w: i32, h: i32) -> Item {
        Item {
            rect: Rectangle::new((x, y).into(), (w, h).into()),
            extents: EXT,
        }
    }
    fn item(w: i32, h: i32) -> Item {
        at(0, 0, w, h)
    }
    fn area() -> Rectangle<i32, Logical> {
        Rectangle::new((0, 0).into(), (1920, 1080).into())
    }

    #[test]
    fn one_small_window_is_not_scaled_up() {
        let s = grid(area(), &[item(400, 300)], 24);
        assert_eq!(s[0].scale, 1.0);
        assert_eq!(s[0].content.size, (400, 300).into());
        assert_eq!(s[0].frame, EXT.around(s[0].content));
        // centred in the area
        assert_eq!(s[0].frame.loc.x + s[0].frame.size.w / 2, 960);
    }

    #[test]
    fn four_equal_windows_make_a_two_by_two_grid() {
        let s = grid(
            area(),
            &[
                item(1920, 1080),
                item(1920, 1080),
                item(1920, 1080),
                item(1920, 1080),
            ],
            24,
        );
        let xs: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.x).collect();
        let ys: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.y).collect();
        assert_eq!((xs.len(), ys.len()), (2, 2));
        let first = s[0].scale;
        assert!(s.iter().all(|x| (x.scale - first).abs() < 1e-9 && x.scale < 0.5));
    }

    #[test]
    fn slots_never_overlap_and_stay_inside_the_area() {
        let items: Vec<_> = (0..7).map(|i| item(300 + 100 * i, 900 - 80 * i)).collect();
        let s = grid(area(), &items, 24);
        for (i, a) in s.iter().enumerate() {
            assert!(area().contains_rect(a.frame));
            for b in &s[i + 1..] {
                assert!(!a.frame.overlaps(b.frame));
            }
        }
    }

    #[test]
    fn small_windows_that_fit_any_way_make_a_square_grid() {
        let s = grid(
            area(),
            &[item(300, 200), item(300, 200), item(300, 200), item(300, 200)],
            24,
        );
        assert!(s.iter().all(|x| x.scale == 1.0));
        let xs: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.x).collect();
        let ys: std::collections::BTreeSet<_> = s.iter().map(|s| s.frame.loc.y).collect();
        assert_eq!((xs.len(), ys.len()), (2, 2));
    }

    #[test]
    fn empty_input_gives_no_slots() {
        assert!(grid(area(), &[], 24).is_empty());
    }

    #[test]
    fn reading_order_is_rows_then_x() {
        let r = |x, y| Rectangle::new((x, y).into(), (100, 100).into());
        assert_eq!(
            reading_order(&[r(900, 0), r(0, 600), r(0, 0), r(900, 600)], 2),
            vec![2, 0, 1, 3]
        );
    }

    #[test]
    fn neighbour_moves_by_direction_and_skips_ineligible() {
        // top-left, top-right, bottom-left, bottom-right: they keep those cells
        let s = grid(
            area(),
            &[
                at(0, 0, 900, 500),
                at(1000, 0, 900, 500),
                at(0, 600, 900, 500),
                at(1000, 600, 900, 500),
            ],
            24,
        );
        assert!(s[0].frame.loc.x < s[1].frame.loc.x && s[0].frame.loc.y < s[2].frame.loc.y);
        assert_eq!(neighbour(&s, 0, Direction::Right, &[true; 4]), Some(1));
        assert_eq!(neighbour(&s, 0, Direction::Down, &[true; 4]), Some(2));
        assert_eq!(neighbour(&s, 0, Direction::Left, &[true; 4]), None);
        assert_eq!(
            neighbour(&s, 0, Direction::Right, &[true, false, true, true]),
            Some(3)
        );
    }
}
