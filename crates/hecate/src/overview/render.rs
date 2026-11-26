//! Drawing the overview: each window's live surfaces scaled into its slot, with a crisp
//! frame drawn at slot size around them.

use std::time::Duration;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::Window;
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle, Scale};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement};
use smithay::utils::Transform;

use super::PromptKey;
use crate::config::AnimStyle;
use crate::render::border::{self, BorderStyle};
use crate::render::fonts::{self, Lookup};
use crate::render::{HecateElement, WindowElement, label, pixelate, snapshot};
use crate::state::Hecate;
use crate::wm::{WindowExt, Wm};

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A window's surfaces with their content's top-left at `content.loc` (relative to
/// `origin`), drawn at `zoom` times their size. Popups are left out.
#[allow(clippy::too_many_arguments)]
fn push_surfaces(
    renderer: &mut GlesRenderer,
    out: &mut Vec<WindowElement>,
    window: &Window,
    content: Rectangle<i32, Logical>,
    origin: Point<i32, Logical>,
    scale: Scale<f64>,
    zoom: f64,
    alpha: f32,
) {
    let geometry = window.geometry().loc.to_f64();
    let loc = (content.loc - origin).to_f64() - Point::from((geometry.x * zoom, geometry.y * zoom));
    let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = render_elements_from_surface_tree(
        renderer,
        window.toplevel_surface().wl_surface(),
        loc.to_physical(scale).to_i32_round(),
        Scale::from(scale.x * zoom),
        alpha,
        Kind::Unspecified,
    );
    out.extend(elems.into_iter().map(WindowElement::Surface));
}

impl Hecate {
    /// The overview on `output`, topmost first, in place of its windows.
    pub(crate) fn push_overview(
        &self,
        renderer: &mut GlesRenderer,
        out: &mut Vec<HecateElement>,
        output: &Output,
        now: Duration,
    ) {
        let o = &self.overview;
        let output_loc = Wm::output_geometry(output).loc;
        let scale = Scale::from(output.current_scale().fractional_scale());
        let p = o.progress(now);
        let closing = !o.open;
        let anim = o.anim.as_ref().filter(|a| !a.is_done(now));
        let style = anim.map(|a| a.style);
        let cell = anim.map_or(1, |a| pixelate::cell_px(a.pixelate_cell(now, closing), scale));

        if o.open && !o.filter.is_empty() {
            self.push_prompt(renderer, out, output);
        }

        // The selected window goes on top of the rest.
        let mut entries: Vec<_> = o.entries(output).iter().collect();
        entries.sort_by_key(|e| Some(&e.window) != o.selected.as_ref());
        for entry in entries {
            let window = &entry.window;
            let slot = entry.slot;
            let size = window.geometry().size;
            let (content, zoom) = match style {
                Some(AnimStyle::Slide) => {
                    let zoom = lerp(1., slot.scale, p);
                    let loc = Point::<f64, Logical>::from((
                        lerp(entry.from.loc.x as f64, slot.content.loc.x as f64, p),
                        lerp(entry.from.loc.y as f64, slot.content.loc.y as f64, p),
                    ));
                    let rect = Rectangle::new(
                        loc.to_i32_round(),
                        (
                            ((size.w as f64 * zoom).round() as i32).max(1),
                            ((size.h as f64 * zoom).round() as i32).max(1),
                        )
                            .into(),
                    );
                    (rect, zoom)
                }
                _ => (slot.content, slot.scale),
            };
            let mut frame_style: BorderStyle = border::style_for(&self.config, window, false);
            if o.selected.as_ref() == Some(window) {
                frame_style.color = self.config.border.tint.gold;
            }
            let alpha = if o.matches(window) {
                1.0
            } else {
                self.config.overview.dim.clamp(0., 1.)
            };

            let mut elems = Vec::new();
            let pixelated = if cell > 1 {
                let frame = frame_style.extents().around(content);
                let cache = window.user_data().get_or_insert(pixelate::PixelCache::default);
                cache.element(
                    renderer,
                    cell,
                    (frame.loc - output_loc).to_physical_precise_round(scale),
                    frame.size,
                    scale,
                    alpha,
                    |renderer, size| {
                        let mut body = Vec::new();
                        push_surfaces(renderer, &mut body, window, content, frame.loc, scale, zoom, 1.0);
                        border::push_frame(
                            renderer,
                            &mut body,
                            window,
                            content,
                            frame.loc,
                            scale,
                            1.0,
                            &frame_style,
                        );
                        snapshot::render_to_texture(renderer, size, scale, &body)
                    },
                )
            } else {
                if let Some(cache) = window.user_data().get::<pixelate::PixelCache>() {
                    cache.clear();
                }
                None
            };
            match pixelated {
                Some(e) => elems.push(WindowElement::Pixelated(e)),
                None => {
                    push_surfaces(
                        renderer, &mut elems, window, content, output_loc, scale, zoom, alpha,
                    );
                    border::push_frame(
                        renderer,
                        &mut elems,
                        window,
                        content,
                        output_loc,
                        scale,
                        alpha,
                        &frame_style,
                    );
                }
            }
            if o.selected.as_ref() == Some(window) && cell == 1 {
                self.push_close_button(
                    renderer,
                    out,
                    frame_style.extents().around(content),
                    output_loc,
                    scale,
                );
            }
            out.extend(elems.into_iter().map(HecateElement::Window));
        }
    }

    /// The selected slot's close button: an `x` in the background colour on love.
    fn push_close_button(
        &self,
        renderer: &mut GlesRenderer,
        out: &mut Vec<HecateElement>,
        frame: Rectangle<i32, Logical>,
        output_loc: Point<i32, Logical>,
        scale: Scale<f64>,
    ) {
        let button = super::layout::close_button(frame);
        let key = PromptKey {
            text: "x".into(),
            max_width: button.size.w as usize,
            fg: self.config.border.background,
            bg: self.config.border.tint.love,
            zoom: scale.x.ceil().max(1.) as usize,
        };
        let mut cache = self.overview.close.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            *cache = label::layout("x", 64).map(|line| {
                // The notch line is padded; keep the 16×16 around the glyph.
                let (pixels, w, _) = label::rasterize_line(&line, key.fg, key.bg, key.zoom);
                let z = key.zoom;
                let side = button.size.w as usize * z;
                let (x0, y0) = ((w - side) / 2, label::NOTCH_PAD_Y * z);
                let mut cut = Vec::with_capacity(side * side * 4);
                for y in y0..y0 + side {
                    cut.extend_from_slice(&pixels[(y * w + x0) * 4..(y * w + x0 + side) * 4]);
                }
                let buf = MemoryRenderBuffer::from_slice(
                    &cut,
                    Fourcc::Argb8888,
                    (side as i32, side as i32),
                    z as i32,
                    Transform::Normal,
                    None,
                );
                (key.clone(), buf)
            });
        }
        let Some((_, buffer)) = cache.as_ref() else {
            return;
        };
        match MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (button.loc - output_loc).to_physical_precise_round(scale),
            buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            Ok(e) => out.push(HecateElement::Memory(e)),
            Err(err) => tracing::warn!("error importing the overview close button: {err:?}"),
        }
    }

    /// The typed filter, `> filter_`, centred in the row kept free above the grid.
    fn push_prompt(&self, renderer: &mut GlesRenderer, out: &mut Vec<HecateElement>, output: &Output) {
        let o = &self.overview;
        let gap = self.config.overview.gap.max(0);
        let Some(&usable) = o.usable.get(&output.name()) else {
            return;
        };
        let output_loc = Wm::output_geometry(output).loc;
        let scale = Scale::from(output.current_scale().fractional_scale());
        // A character the font can't draw shows as `?`; the filter keeps the real one.
        let shown: String = o
            .filter
            .chars()
            .map(|c| match fonts::lookup(c) {
                Lookup::Missing => '?',
                _ => c,
            })
            .collect();
        let key = PromptKey {
            text: format!("> {shown}_"),
            max_width: (usable.size.w - 2 * gap).max(0) as usize,
            fg: self.config.border.label.color,
            bg: self.config.border.background,
            zoom: scale.x.ceil().max(1.) as usize,
        };
        let mut cache = o.prompt.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _, _)| *k != key) {
            let inner = key.max_width.saturating_sub(2 * label::NOTCH_PAD_X);
            *cache = label::layout(&key.text, inner).map(|line| {
                let (pixels, w, h) = label::rasterize_line(&line, key.fg, key.bg, key.zoom);
                let buf = MemoryRenderBuffer::from_slice(
                    &pixels,
                    Fourcc::Argb8888,
                    (w as i32, h as i32),
                    key.zoom as i32,
                    Transform::Normal,
                    None,
                );
                (key.clone(), buf, (line.width + 2 * label::NOTCH_PAD_X) as i32)
            });
        }
        let Some((_, buffer, width)) = cache.as_ref() else {
            return;
        };
        let loc =
            Point::<i32, Logical>::from((usable.loc.x + (usable.size.w - width) / 2, usable.loc.y + gap));
        match MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            (loc - output_loc).to_physical_precise_round(scale),
            buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            Ok(e) => out.push(HecateElement::Memory(e)),
            Err(err) => tracing::warn!("error importing the overview prompt: {err:?}"),
        }
    }
}
