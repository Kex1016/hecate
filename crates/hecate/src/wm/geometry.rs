//! Geometry helpers: output rects, placement, snapping.

use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::config::action::Edge;

pub fn output_geometry(output: &Output) -> Rectangle<i32, Logical> {
    let loc = output.current_location();
    let size = output
        .current_mode()
        .map(|m| {
            let transform = output.current_transform();
            let scale = output.current_scale().fractional_scale();
            transform
                .transform_size(m.size)
                .to_f64()
                .to_logical(scale)
                .to_i32_round()
        })
        .unwrap_or_else(|| Size::from((1920, 1080)));
    Rectangle::new(loc, size)
}

pub fn center(r: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    Point::from((r.loc.x + r.size.w / 2, r.loc.y + r.size.h / 2))
}

/// Centre a size inside an area, clamped so the top-left stays visible.
pub fn centered(area: Rectangle<i32, Logical>, size: Size<i32, Logical>) -> Point<i32, Logical> {
    let x = area.loc.x + (area.size.w - size.w) / 2;
    let y = area.loc.y + (area.size.h - size.h) / 2;
    Point::from((x.max(area.loc.x), y.max(area.loc.y)))
}

/// Clamp a rect's position so it stays inside `area` as far as possible.
pub fn clamp_into(area: Rectangle<i32, Logical>, rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    let max_x = (area.loc.x + area.size.w - rect.size.w).max(area.loc.x);
    let max_y = (area.loc.y + area.size.h - rect.size.h).max(area.loc.y);
    Point::from((
        rect.loc.x.clamp(area.loc.x, max_x),
        rect.loc.y.clamp(area.loc.y, max_y),
    ))
}

/// The rect a window takes when snapped to an edge or corner, inside `area` with `gap`.
pub fn snap_rect(area: Rectangle<i32, Logical>, edge: Edge, gap: i32) -> Rectangle<i32, Logical> {
    let a = shrink(area, gap);
    let half_w = (a.size.w - gap) / 2;
    let half_h = (a.size.h - gap) / 2;
    let right_x = a.loc.x + a.size.w - half_w;
    let bottom_y = a.loc.y + a.size.h - half_h;
    let (x, y, w, h) = match edge {
        Edge::Left => (a.loc.x, a.loc.y, half_w, a.size.h),
        Edge::Right => (right_x, a.loc.y, half_w, a.size.h),
        Edge::Top => (a.loc.x, a.loc.y, a.size.w, half_h),
        Edge::Bottom => (a.loc.x, bottom_y, a.size.w, half_h),
        Edge::TopLeft => (a.loc.x, a.loc.y, half_w, half_h),
        Edge::TopRight => (right_x, a.loc.y, half_w, half_h),
        Edge::BottomLeft => (a.loc.x, bottom_y, half_w, half_h),
        Edge::BottomRight => (right_x, bottom_y, half_w, half_h),
    };
    Rectangle::new((x, y).into(), (w.max(1), h.max(1)).into())
}

pub fn shrink(r: Rectangle<i32, Logical>, by: i32) -> Rectangle<i32, Logical> {
    Rectangle::new(
        (r.loc.x + by, r.loc.y + by).into(),
        ((r.size.w - 2 * by).max(1), (r.size.h - 2 * by).max(1)).into(),
    )
}

/// Place a window of `size` next to existing rects, minimising overlap ("automatic" policy).
pub fn place_automatic(
    area: Rectangle<i32, Logical>,
    size: Size<i32, Logical>,
    existing: &[Rectangle<i32, Logical>],
) -> Point<i32, Logical> {
    let step = 32;
    let mut best = centered(area, size);
    let mut best_overlap = i64::MAX;
    let max_x = (area.loc.x + area.size.w - size.w).max(area.loc.x);
    let max_y = (area.loc.y + area.size.h - size.h).max(area.loc.y);
    let mut y = area.loc.y;
    while y <= max_y {
        let mut x = area.loc.x;
        while x <= max_x {
            let cand = Rectangle::new((x, y).into(), size);
            let overlap: i64 = existing
                .iter()
                .filter_map(|r| r.intersection(cand))
                .map(|r| r.size.w as i64 * r.size.h as i64)
                .sum();
            if overlap < best_overlap {
                best_overlap = overlap;
                best = cand.loc;
                if overlap == 0 {
                    return best;
                }
            }
            x += step;
        }
        y += step;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn snapping() {
        let area = r(0, 0, 1000, 800);
        assert_eq!(snap_rect(area, Edge::Left, 0), r(0, 0, 500, 800));
        assert_eq!(snap_rect(area, Edge::Right, 0), r(500, 0, 500, 800));
        assert_eq!(snap_rect(area, Edge::BottomRight, 0), r(500, 400, 500, 400));
        let g = snap_rect(area, Edge::Left, 10);
        assert_eq!(g, r(10, 10, 485, 780));
        let g2 = snap_rect(area, Edge::Right, 10);
        assert_eq!(g2.loc.x + g2.size.w, 990);
        assert_eq!(g2.loc.x - (g.loc.x + g.size.w), 10);
    }

    #[test]
    fn centering_and_clamping() {
        let area = r(100, 0, 1000, 800);
        assert_eq!(centered(area, (200, 100).into()), Point::from((500, 350)));
        assert_eq!(centered(area, (2000, 100).into()), Point::from((100, 350)));
        assert_eq!(clamp_into(area, r(1050, -5, 200, 100)), Point::from((900, 0)));
    }

    #[test]
    fn automatic_avoids_overlap() {
        let area = r(0, 0, 1000, 800);
        let p = place_automatic(area, (400, 400).into(), &[r(0, 0, 500, 800)]);
        assert!(p.x >= 500);
    }
}
