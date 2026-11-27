//! Drawing the overview: each window's live surfaces scaled into its slot, with a crisp
//! frame drawn at slot size around them.

use std::time::Duration;

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{Element, Id, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::{GlesError, GlesFrame, GlesRenderer};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::desktop::Window;
use smithay::output::Output;
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Physical};
use smithay::utils::{Logical, Point, Rectangle, Scale};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::utils::Transform;

use super::PromptKey;
use crate::config::AnimStyle;
use crate::render::border::{self, BorderStyle};
use crate::render::fonts::{self, Lookup};
use crate::render::{HecateElement, WindowElement, label, pixelate, snapshot};
use crate::state::Hecate;
use crate::wm::{WindowExt, Wm};

/// A surface element under another id. The strip draws windows that the grid draws too,
/// and two elements with one id in a frame confuse damage tracking: the second one took
/// the first one's place.
#[derive(Debug)]
pub struct Again {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    id: Id,
}

impl Element for Again {
    fn id(&self) -> &Id {
        &self.id
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        self.inner.location(scale)
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn transform(&self) -> smithay::utils::Transform {
        self.inner.transform()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        self.inner.opaque_regions(scale)
    }
    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for Again {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        self.inner.draw(frame, src, dst, damage, opaque_regions, cache)
    }
    fn underlying_storage(&self, renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        self.inner.underlying_storage(renderer)
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Ids for a second drawing of surfaces: per strip thumbnail, per surface element.
type AgainIds = std::cell::RefCell<std::collections::HashMap<(u64, Id), Id>>;

/// A window's surfaces with their content's top-left at `content.loc` (relative to
/// `origin`), drawn at `zoom` times their size. Popups are left out. With `again`, the
/// elements get ids of their own for that thumbnail (the grid draws them too).
#[allow(clippy::too_many_arguments)]
fn push_surfaces(
    renderer: &mut GlesRenderer,
    out: &mut Vec<HecateElement>,
    window: &Window,
    content: Rectangle<i32, Logical>,
    origin: Point<i32, Logical>,
    scale: Scale<f64>,
    zoom: f64,
    alpha: f32,
    again: Option<(&AgainIds, u64)>,
) {
    // Drawn at full size with the content's top-left at its place, then scaled about it:
    // a surface element's size follows the output scale alone.
    let anchor = content.loc - origin;
    let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = render_elements_from_surface_tree(
        renderer,
        window.toplevel_surface().wl_surface(),
        (anchor - window.geometry().loc).to_physical_precise_round(scale),
        scale,
        alpha,
        Kind::Unspecified,
    );
    let anchor = anchor.to_physical_precise_round(scale);
    for e in elems {
        let e = match again {
            Some((ids, thumb)) => {
                let id = ids
                    .borrow_mut()
                    .entry((thumb, e.id().clone()))
                    .or_insert_with(Id::new)
                    .clone();
                WindowElement::Again(Again { inner: e, id })
            }
            None => WindowElement::Surface(e),
        };
        out.push(HecateElement::Scaled(RescaleRenderElement::from_element(
            e, anchor, zoom,
        )));
    }
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

        // A window being dragged follows the pointer, over everything.
        let dragged = o.drag.as_ref().filter(|d| d.active);
        if let Some(d) = dragged
            && let Some(e) = o.entries(output).iter().find(|e| e.window == d.window)
        {
            let shift = d.pos - d.start;
            let loc = (e.slot.content.loc.to_f64() + shift).to_i32_round();
            let content = Rectangle::new(loc, e.slot.content.size);
            push_surfaces(
                renderer,
                out,
                &e.window,
                content,
                output_loc,
                scale,
                e.slot.scale,
                0.8,
                None,
            );
        }

        // The selected window goes on top of the rest.
        let mut entries: Vec<_> = o
            .entries(output)
            .iter()
            .filter(|e| dragged.is_none_or(|d| d.window != e.window))
            .collect();
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
            // Every slot has a frame, also where the desktop draws none (maximized,
            // fullscreen): its colour is what shows the selection.
            let selected = o.selected.as_ref() == Some(window);
            let framed = border::style_for_overview(&self.config, window);
            let mut frame_style: BorderStyle = framed.style;
            if selected {
                frame_style.color = self.config.border.tint.gold;
            }
            let draw_frame = selected || !framed.selected_only;
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
                        push_surfaces(
                            renderer, &mut body, window, content, frame.loc, scale, zoom, 1.0, None,
                        );
                        let mut lines = Vec::new();
                        if draw_frame {
                            border::push_frame(
                                renderer,
                                &mut lines,
                                window,
                                content,
                                frame.loc,
                                scale,
                                1.0,
                                &frame_style,
                            );
                        }
                        body.extend(lines.into_iter().map(HecateElement::Window));
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
                Some(e) => elems.push(HecateElement::Window(WindowElement::Pixelated(e))),
                None => {
                    push_surfaces(
                        renderer, &mut elems, window, content, output_loc, scale, zoom, alpha, None,
                    );
                    let mut lines = Vec::new();
                    if draw_frame {
                        border::push_frame(
                            renderer,
                            &mut lines,
                            window,
                            content,
                            output_loc,
                            scale,
                            alpha,
                            &frame_style,
                        );
                    }
                    elems.extend(lines.into_iter().map(HecateElement::Window));
                }
            }
            if selected && cell == 1 {
                self.push_close_button(
                    renderer,
                    out,
                    frame_style.extents().around(content),
                    output_loc,
                    scale,
                );
            }
            out.extend(elems);
        }

        self.push_strip(renderer, out, output, p.clamp(0., 1.) as f32);
    }

    /// The workspace strip: each workspace's windows, small, in a thin frame with its
    /// number. Gold for the active one, foam for where a dragged window would go.
    fn push_strip(
        &self,
        renderer: &mut GlesRenderer,
        out: &mut Vec<HecateElement>,
        output: &Output,
        alpha: f32,
    ) {
        let o = &self.overview;
        let Some(thumbs) = o.strips.get(&output.name()) else {
            return;
        };
        let output_geo = Wm::output_geometry(output);
        let scale = Scale::from(output.current_scale().fractional_scale());
        let active = self.wm.active_workspace_id(output);
        let target = o
            .drag
            .as_ref()
            .filter(|d| d.active)
            .and_then(|d| thumbs.iter().find(|t| t.frame.to_f64().contains(d.pos)))
            .map(|t| t.workspace);
        let colors = &self.config.border;
        let mut solids = o.solids.borrow_mut();
        let mut solid = |out: &mut Vec<HecateElement>,
                         key: (u64, u8),
                         rect: Rectangle<i32, Logical>,
                         color: crate::config::Color| {
            let buffer = solids
                .entry(key)
                .or_insert_with(|| SolidColorBuffer::new(rect.size, color.premultiplied()));
            buffer.update(rect.size, color.premultiplied());
            out.push(HecateElement::Solid(SolidColorRenderElement::from_buffer(
                buffer,
                (rect.loc - output_geo.loc).to_physical_precise_round(scale),
                scale,
                alpha,
                Kind::Unspecified,
            )));
        };
        let zoom = scale.x.ceil().max(1.) as usize;
        for t in thumbs {
            let line = if Some(t.workspace) == target {
                colors.tint.foam
            } else if t.workspace == active {
                colors.tint.gold
            } else {
                colors.color.inactive
            };
            // The number, on the content's top-left corner.
            let text = self.wm.workspace_label(t.workspace);
            {
                let mut numbers = o.numbers.borrow_mut();
                let buffer = numbers.entry((text.clone(), zoom)).or_insert_with(|| {
                    let line = label::layout(&text, 64)
                        .or_else(|| label::layout(&t.index.to_string(), 64))
                        .expect("digits have glyphs");
                    let (pixels, w, h) =
                        label::rasterize_line(&line, colors.label.color, colors.background, zoom);
                    MemoryRenderBuffer::from_slice(
                        &pixels,
                        Fourcc::Argb8888,
                        (w as i32, h as i32),
                        zoom as i32,
                        Transform::Normal,
                        None,
                    )
                });
                if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    (t.content.loc - output_geo.loc).to_physical_precise_round(scale),
                    buffer,
                    Some(alpha),
                    None,
                    None,
                    Kind::Unspecified,
                ) {
                    out.push(HecateElement::Memory(e));
                }
            }
            // Four lines around the content.
            let (f, e) = (t.frame, super::THUMB_EXTENTS);
            let ws = t.workspace;
            let inner_h = f.size.h - e.top - e.bottom;
            solid(
                out,
                (ws, 0),
                Rectangle::new(f.loc, (f.size.w, e.top).into()),
                line,
            );
            solid(
                out,
                (ws, 1),
                Rectangle::new(
                    (f.loc.x, f.loc.y + f.size.h - e.bottom).into(),
                    (f.size.w, e.bottom).into(),
                ),
                line,
            );
            solid(
                out,
                (ws, 2),
                Rectangle::new((f.loc.x, f.loc.y + e.top).into(), (e.left, inner_h).into()),
                line,
            );
            solid(
                out,
                (ws, 3),
                Rectangle::new(
                    (f.loc.x + f.size.w - e.right, f.loc.y + e.top).into(),
                    (e.right, inner_h).into(),
                ),
                line,
            );
            // The workspace's windows, where they are on the output, topmost first.
            if let Some(w) = self.wm.workspace(ws) {
                for window in w.windows.iter().rev() {
                    if self.wm.output_for_window(window) != Some(output) {
                        continue;
                    }
                    let rect = self.wm.window_rect(window);
                    let rel = rect.loc - output_geo.loc;
                    let content = Rectangle::new(
                        (
                            t.content.loc.x + (rel.x as f64 * t.scale).round() as i32,
                            t.content.loc.y + (rel.y as f64 * t.scale).round() as i32,
                        )
                            .into(),
                        (
                            ((rect.size.w as f64 * t.scale).round() as i32).max(1),
                            ((rect.size.h as f64 * t.scale).round() as i32).max(1),
                        )
                            .into(),
                    );
                    push_surfaces(
                        renderer,
                        out,
                        window,
                        content,
                        output_geo.loc,
                        scale,
                        t.scale,
                        alpha,
                        Some((&o.ids, t.workspace)),
                    );
                }
            }
            solid(out, (ws, 4), t.content, colors.background);
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
        let y = super::Rows::new(usable, gap, self.config.overview.strip).prompt_y;
        let loc = Point::<i32, Logical>::from((usable.loc.x + (usable.size.w - width) / 2, y));
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
