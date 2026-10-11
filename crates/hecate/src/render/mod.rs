//! Building the list of render elements for an output.

pub mod blur;
pub mod border;
pub mod capture;
pub mod screencopy;

use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Kind};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::render_elements;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale, Transform};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;

use crate::anim::Animation;
use crate::config::{Color, Config, LayerRule};
use crate::cursor::XCursor;
use crate::state::Hecate;
use crate::wm::{WindowExt, Wm};

render_elements! {
    pub HecateElement<=GlesRenderer>;
    Surface = WaylandSurfaceRenderElement<GlesRenderer>,
    Solid = SolidColorRenderElement,
    Memory = MemoryRenderBufferRenderElement<GlesRenderer>,
    Window = WindowElement,
    Scaled = RescaleRenderElement<WindowElement>,
    Blur = blur::BlurElement,
}

/// A layer surface that wants blur behind it, found while building the element list.
struct BlurMarker {
    /// Index in the element list just below the surface's elements.
    index: usize,
    /// Stable element id (damage tracking).
    id: smithay::backend::renderer::element::Id,
    /// The layer surface, for its `ignore_alpha` mask.
    surface: Option<smithay::desktop::LayerSurface>,
    geometry: Rectangle<i32, Physical>,
    alpha: f32,
    xray: bool,
    ignore_alpha: Option<f32>,
    /// Clip to a rounded rectangle, logical and relative to `geometry`.
    rounding: Option<(Rectangle<f64, Logical>, crate::config::Corners)>,
}

#[allow(clippy::too_many_arguments)]
/// Blur markers for a surface's ext-background-effect region (surface-local, placed at
/// `origin` in output logical coordinates), clipped to `clip`.
fn region_markers(
    surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    origin: Point<i32, Logical>,
    clip: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    index: usize,
    layer: Option<&smithay::desktop::LayerSurface>,
    alpha: f32,
    rounding: Option<crate::config::Corners>,
) -> Vec<BlurMarker> {
    let Some(region) = crate::protocols::background_effect::blur_region(surface) else {
        return Vec::new();
    };
    region
        .into_iter()
        .filter_map(|r| Rectangle::new(r.loc + origin, r.size).intersection(clip))
        .enumerate()
        .map(|(i, r)| {
            let geometry = r.to_physical_precise_round(scale);
            BlurMarker {
                id: blur_id(surface, i + 1),
                index,
                surface: layer.cloned(),
                geometry,
                alpha,
                xray: false,
                ignore_alpha: None,
                rounding: rounding.filter(|c| !c.is_zero()).map(|c| {
                    // The clip rect relative to this piece of the region.
                    let rel = Rectangle::new((clip.loc - r.loc).to_f64(), clip.size.to_f64());
                    (rel, c)
                }),
            }
        })
        .collect()
}

/// Stable element ids for a surface's blur: one per layer rule, or per region piece.
#[derive(Default)]
struct BlurIds(std::sync::Mutex<Vec<smithay::backend::renderer::element::Id>>);

fn blur_id(
    surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    i: usize,
) -> smithay::backend::renderer::element::Id {
    with_states(surface, |states| {
        let ids = states.data_map.get_or_insert_threadsafe(BlurIds::default);
        let mut ids = ids.0.lock().unwrap();
        while ids.len() <= i {
            ids.push(smithay::backend::renderer::element::Id::new());
        }
        ids[i].clone()
    })
}

render_elements! {
    /// What a window (or its snapshot) is drawn with; scaled as a whole by animations.
    pub WindowElement<=GlesRenderer>;
    Surface = WaylandSurfaceRenderElement<GlesRenderer>,
    Border = border::BorderElement,
    Clipped = border::ClippedSurfaceElement,
    Texture = TextureRenderElement<GlesTexture>,
}

/// Push window elements, scaled around `origin` unless the scale is 1.
fn push_scaled(
    out: &mut Vec<HecateElement>,
    elems: Vec<WindowElement>,
    origin: Point<i32, Physical>,
    scale: f64,
) {
    if (scale - 1.).abs() < 1e-4 {
        out.extend(elems.into_iter().map(HecateElement::Window));
    } else {
        out.extend(
            elems
                .into_iter()
                .map(|e| HecateElement::Scaled(RescaleRenderElement::from_element(e, origin, scale))),
        );
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0., 1.);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// Effective layer rule properties for a namespace (later rules win).
pub fn layer_rules_for(config: &Config, namespace: &str) -> LayerRule {
    let mut out = LayerRule::default();
    for rule in &config.layer_rules {
        let matches = rule
            .matches
            .namespace
            .as_ref()
            .is_none_or(|re| regex::Regex::new(re).is_ok_and(|re| re.is_match(namespace)));
        if !matches {
            continue;
        }
        macro_rules! take {
            ($($f:ident),*) => { $( if rule.$f.is_some() { out.$f = rule.$f.clone(); } )* };
        }
        take!(
            blur,
            blur_popups,
            ignore_alpha,
            xray,
            no_keyboard_focus,
            opacity,
            animation
        );
    }
    out
}

impl Hecate {
    /// Render elements for an output, topmost first.
    pub fn render_elements(&self, renderer: &mut GlesRenderer, output: &Output) -> Vec<HecateElement> {
        self.render_elements_for(renderer, output, true)
    }

    /// Render elements for an output, topmost first, optionally without the cursor (captures).
    pub fn render_elements_for(
        &self,
        renderer: &mut GlesRenderer,
        output: &Output,
        include_cursor: bool,
    ) -> Vec<HecateElement> {
        let mut out: Vec<HecateElement> = Vec::new();
        let scale = Scale::from(output.current_scale().fractional_scale());
        let output_geo = Wm::output_geometry(output);
        let to_local = |p: Point<i32, Logical>| -> Point<i32, Physical> {
            (p - output_geo.loc).to_physical_precise_round(scale)
        };
        let state = self.output_state.get(output);

        // Cursor on top.
        if include_cursor {
            self.push_cursor(renderer, output, &mut out);
        }

        if self.wm.is_locked() {
            if let Some(lock) = self.wm.lock_surface(output) {
                out.extend(
                    render_elements_from_surface_tree(
                        renderer,
                        lock.wl_surface(),
                        Point::from((0, 0)),
                        scale,
                        1.0,
                        Kind::ScanoutCandidate,
                    )
                    .into_iter()
                    .map(HecateElement::Surface),
                );
            }
            if let Some(s) = state {
                out.push(HecateElement::Solid(SolidColorRenderElement::from_buffer(
                    &s.lock_background,
                    (0, 0),
                    scale,
                    1.0,
                    Kind::Unspecified,
                )));
            }
            return out;
        }

        let map = layer_map_for_output(output);
        let now = self.wm.clock.now();
        let mut markers: Vec<BlurMarker> = Vec::new();
        let push_layer = |out: &mut Vec<HecateElement>,
                          markers: &mut Vec<BlurMarker>,
                          renderer: &mut GlesRenderer,
                          layer: Layer| {
            self.push_closing(out, output, Some(layer), now);
            for surface in map.layers_on(layer).rev() {
                let Some(geo) = map.layer_geometry(surface) else {
                    continue;
                };
                let rules = layer_rules_for(&self.config, surface.namespace());
                let (anim_scale, anim_alpha, slide) = surface
                    .user_data()
                    .get::<std::cell::RefCell<crate::anim::LayerAnimState>>()
                    .and_then(|s| {
                        let s = s.borrow();
                        let (a, from) = s.open.as_ref()?;
                        let (sc, al) = a.scale_alpha(now, false);
                        let k = 1. - a.value(now);
                        Some((sc, al, Point::<f64, Logical>::from((from.x * k, from.y * k))))
                    })
                    .unwrap_or((1., 1., Point::default()));
                let alpha = rules.opacity.unwrap_or(1.0).clamp(0., 1.) * anim_alpha;
                let geo_loc = (geo.loc.to_f64() + slide).to_i32_round::<i32>();
                let loc = geo_loc.to_physical_precise_round(scale);
                let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    surface.render_elements(renderer, loc, scale, alpha);
                let center = geo_loc.to_f64() + geo.size.to_f64().downscale(2.);
                push_scaled(
                    out,
                    elems.into_iter().map(WindowElement::Surface).collect(),
                    center.to_physical_precise_round(scale),
                    anim_scale,
                );
                if !self.config.blur.enable {
                    continue;
                }
                if rules.blur == Some(true) {
                    markers.push(BlurMarker {
                        id: blur_id(surface.wl_surface(), 0),
                        index: out.len(),
                        surface: Some(surface.clone()),
                        geometry: Rectangle::new(geo_loc, geo.size).to_physical_precise_round(scale),
                        alpha,
                        xray: rules.xray.unwrap_or(false),
                        ignore_alpha: rules.ignore_alpha,
                        rounding: None,
                    });
                } else if rules.blur != Some(false) {
                    markers.extend(region_markers(
                        surface.wl_surface(),
                        geo_loc,
                        Rectangle::new(geo_loc, geo.size),
                        scale,
                        out.len(),
                        Some(surface),
                        alpha,
                        None,
                    ));
                }
            }
        };

        push_layer(&mut out, &mut markers, renderer, Layer::Overlay);

        let windows = self.wm.windows_on_output(output);
        let fullscreen = windows.iter().any(|w| w.data().fullscreen);
        if !fullscreen {
            push_layer(&mut out, &mut markers, renderer, Layer::Top);
        }

        let focused = self
            .seat
            .get_keyboard()
            .and_then(|k| k.current_focus())
            .map(|s| crate::utils::root_surface(&s));
        self.push_closing(&mut out, output, None, now);
        // A workspace switch slides the new workspace in over the old one's way out.
        let width = output_geo.size.w as f64;
        let switch = self.wm.switch_for(output).cloned();
        let (in_offset, out_offset) = match &switch {
            Some(s) => {
                let v = s.anim.value(now);
                let d = s.direction as f64;
                (d * width * (1. - v), -d * width * v)
            }
            None => (0., 0.),
        };
        for window in windows.iter().rev() {
            let is_focused = focused.as_ref() == Some(window.toplevel_surface().wl_surface());
            self.push_window(
                renderer,
                &mut out,
                window,
                is_focused,
                (in_offset, 0.).into(),
                output,
                now,
                &mut markers,
            );
        }
        if let Some(s) = &switch
            && let Some(ws) = self.wm.workspace(s.from)
        {
            for window in ws.windows.iter().rev() {
                if !self.wm.window_rect(window).overlaps(output_geo) {
                    continue;
                }
                self.push_window(
                    renderer,
                    &mut out,
                    window,
                    false,
                    (out_offset, 0.).into(),
                    output,
                    now,
                    &mut markers,
                );
            }
        }

        if fullscreen {
            push_layer(&mut out, &mut markers, renderer, Layer::Top);
        }
        let xray_start = out.len();
        push_layer(&mut out, &mut markers, renderer, Layer::Bottom);
        push_layer(&mut out, &mut markers, renderer, Layer::Background);
        drop(map);

        if let Some(s) = state {
            out.push(HecateElement::Solid(SolidColorRenderElement::from_buffer(
                &s.background,
                (0, 0),
                scale,
                1.0,
                Kind::Unspecified,
            )));
        }
        let _ = to_local;
        self.insert_blur(renderer, output, &mut out, markers, xray_start);
        out
    }

    /// Put the blurred scene below each surface that asked for it.
    fn insert_blur(
        &self,
        renderer: &mut GlesRenderer,
        output: &Output,
        out: &mut Vec<HecateElement>,
        markers: Vec<BlurMarker>,
        mut xray_start: usize,
    ) {
        use smithay::backend::renderer::utils::with_renderer_surface_state;
        let cache = output
            .user_data()
            .get_or_insert(|| std::cell::RefCell::new(blur::BlurCache::default()));
        let mut cache = cache.borrow_mut();
        let scale = Scale::from(output.current_scale().fractional_scale());
        let size = Wm::output_geometry(output).size.to_physical_precise_round(scale);
        let config = &self.config.blur;
        let Some(programs) = blur::BlurShaders::composite(renderer) else {
            return;
        };
        // Bottom-most first, so a lower surface's blur is part of the scene above it.
        for m in markers.into_iter().rev() {
            let start = if m.xray { m.index.max(xray_start) } else { m.index };
            let below = &out[start.min(out.len())..];
            let key = blur::scene_key(below, scale, size, config);
            let Some((texture, commit)) = cache.get(renderer, key, size, scale, below, config) else {
                continue;
            };
            let context = smithay::backend::renderer::Renderer::context_id(renderer);
            let mut mask = None;
            if let Some(threshold) = m.ignore_alpha
                && let Some(surface) = &m.surface
            {
                let (texture, single_pixel_alpha) = with_renderer_surface_state(surface.wl_surface(), |s| {
                    let texture = s.texture::<GlesTexture>(context.clone()).cloned();
                    // Single-pixel buffers are drawn without a texture: their alpha decides.
                    let alpha = s.buffer().and_then(|b| {
                        smithay::wayland::single_pixel_buffer::get_single_pixel_buffer(b)
                            .ok()
                            .map(|d| d.a as f64 / u32::MAX as f64)
                    });
                    (texture, alpha)
                })
                .unwrap_or_default();
                match single_pixel_alpha {
                    Some(a) if a <= threshold as f64 => continue,
                    Some(_) => {}
                    None => mask = texture.map(|t| (t, threshold)),
                }
            }
            let mut elem = blur::BlurElement::new(
                programs.clone(),
                m.id,
                commit,
                texture,
                m.geometry,
                mask,
                config.noise,
                m.alpha,
            );
            if let Some((rect, radius)) = m.rounding {
                elem = elem.with_rounding(rect, radius, scale.x);
            }
            out.insert(m.index, HecateElement::Blur(elem));
            if m.index <= xray_start {
                xray_start += 1;
            }
        }
        cache.end_frame();
    }

    /// One window with its animations applied, shifted by `offset` (workspace switch).
    #[allow(clippy::too_many_arguments)]
    fn push_window(
        &self,
        renderer: &mut GlesRenderer,
        out: &mut Vec<HecateElement>,
        window: &smithay::desktop::Window,
        is_focused: bool,
        offset: Point<f64, Logical>,
        output: &Output,
        now: std::time::Duration,
        markers: &mut Vec<BlurMarker>,
    ) {
        let output_geo = Wm::output_geometry(output);
        let scale = Scale::from(output.current_scale().fractional_scale());
        let mut style = if self.identify_until.is_some() {
            border::style_for_identify(&self.config, window, is_focused)
        } else {
            border::style_for(&self.config, window, is_focused)
        };
        let geo_rect = self.wm.window_rect(window);
        let (anim_scale, anim_alpha, move_offset) = {
            let mut d = window.data();
            let animate = !d.rules.no_animations;
            let target = d.loc;
            let ws = d.workspace;
            let interactive = d.moving || d.resizing;
            let a = &mut d.anims;
            let current_move = |a: &crate::wm::window::WindowAnims| {
                a.moving
                    .as_ref()
                    .map(|(m, off)| {
                        let k = 1. - m.value(now);
                        Point::<f64, Logical>::from((off.x * k, off.y * k))
                    })
                    .unwrap_or_default()
            };
            // A programmatic move (snap, maximize, centre, …) slides from where it was shown.
            if let Some((last, last_ws)) = a.last_target
                && last != target
                && last_ws == ws
            {
                let shown = last.to_f64() + current_move(a);
                a.moving = if animate && !interactive {
                    Animation::start(&self.wm.clock, &self.wm.anim, &self.wm.anim.window_move)
                        .map(|m| (m, shown - target.to_f64()))
                } else {
                    None
                };
            }
            a.last_target = Some((target, ws));
            // Border colour changes (focus, urgency) fade.
            if let Some(last) = a.last_border_color
                && last != style.color
            {
                let shown = match &a.border {
                    Some((b, from)) => lerp_color(*from, last, b.value(now) as f32),
                    None => last,
                };
                a.border = if animate {
                    Animation::start(&self.wm.clock, &self.wm.anim, &self.wm.anim.border).map(|b| (b, shown))
                } else {
                    None
                };
            }
            a.last_border_color = Some(style.color);
            if let Some((b, from)) = &a.border {
                style.color = lerp_color(*from, style.color, b.value(now) as f32);
            }
            let (s, al) = a
                .open
                .as_ref()
                .map(|o| o.scale_alpha(now, false))
                .unwrap_or((1., 1.));
            (s, al, current_move(a))
        };
        let alpha = window.data().rules.opacity.unwrap_or(1.0).clamp(0., 1.) * anim_alpha;
        let shift = offset + move_offset;
        let rect = Rectangle::new((geo_rect.loc.to_f64() + shift).to_i32_round(), geo_rect.size);
        let mut elems = Vec::new();
        border::push_window(
            renderer,
            &mut elems,
            window,
            rect,
            output_geo.loc,
            scale,
            alpha,
            &style,
        );
        if self.config.blur.enable {
            let local = Rectangle::new(rect.loc - output_geo.loc, rect.size);
            markers.extend(region_markers(
                window.toplevel_surface().wl_surface(),
                local.loc - window.geometry().loc,
                local,
                scale,
                out.len() + elems.len(),
                None,
                alpha,
                Some(style.radius),
            ));
        }
        let center = (rect.loc - output_geo.loc).to_f64() + rect.size.to_f64().downscale(2.);
        push_scaled(out, elems, center.to_physical_precise_round(scale), anim_scale);
    }

    /// Closed windows' snapshots playing their close animation.
    fn push_closing(
        &self,
        out: &mut Vec<HecateElement>,
        output: &Output,
        layer: Option<Layer>,
        now: std::time::Duration,
    ) {
        let output_geo = Wm::output_geometry(output);
        let scale = Scale::from(output.current_scale().fractional_scale());
        for c in self.wm.closing.iter().rev() {
            if c.layer != layer
                || !c.rect.overlaps(output_geo)
                || c.workspace.is_some_and(|ws| !self.wm.is_workspace_active(ws))
            {
                continue;
            }
            let (s, alpha) = c.anim.scale_alpha(now, true);
            let v = c.anim.value(now);
            let slide = Point::<f64, Logical>::from((c.slide.x * v, c.slide.y * v));
            let rect_loc = (c.rect.loc.to_f64() + slide).to_i32_round();
            let loc: Point<i32, Physical> = (rect_loc - output_geo.loc).to_physical_precise_round(scale);
            // The texture is in physical pixels; show all of it over the logical rect.
            let tex_size = smithay::backend::renderer::Texture::size(&c.texture);
            let src = Rectangle::from_size((tex_size.w as f64, tex_size.h as f64).into());
            let elem = TextureRenderElement::from_static_texture(
                c.id.clone(),
                c.context.clone(),
                loc.to_f64(),
                c.texture.clone(),
                1,
                Transform::Normal,
                Some(alpha),
                Some(src),
                Some(c.rect.size),
                None,
                Kind::Unspecified,
            );
            let center = (rect_loc - output_geo.loc).to_f64() + c.rect.size.to_f64().downscale(2.);
            push_scaled(
                out,
                vec![WindowElement::Texture(elem)],
                center.to_physical_precise_round(scale),
                s,
            );
        }
    }

    fn push_cursor(&self, renderer: &mut GlesRenderer, output: &Output, out: &mut Vec<HecateElement>) {
        let output_geo = Wm::output_geometry(output);
        let pos = self.pointer_location;
        if !output_geo.to_f64().contains(pos) {
            return;
        }
        let scale = Scale::from(output.current_scale().fractional_scale());
        let int_scale = output.current_scale().integer_scale();
        let local = pos - output_geo.loc.to_f64();

        if let Some(icon) = &self.dnd_icon {
            out.extend(
                render_elements_from_surface_tree(
                    renderer,
                    icon,
                    local.to_physical_precise_round(scale),
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
                .into_iter()
                .map(HecateElement::Surface),
            );
        }

        match &self.cursor_status {
            CursorImageStatus::Hidden => {}
            CursorImageStatus::Surface(surface) => {
                let hotspot = with_states(surface, |states| {
                    states
                        .data_map
                        .get::<CursorImageSurfaceData>()
                        .map(|d| d.lock().unwrap().hotspot)
                        .unwrap_or_default()
                });
                let p = (local - hotspot.to_f64()).to_physical_precise_round(scale);
                out.extend(
                    render_elements_from_surface_tree(renderer, surface, p, scale, 1.0, Kind::Cursor)
                        .into_iter()
                        .map(HecateElement::Surface),
                );
            }
            CursorImageStatus::Named(icon) => {
                let cursor = self.cursor_manager.get(*icon, int_scale);
                let (idx, frame) = cursor.frame(self.start_time.elapsed().as_millis() as u32);
                let hotspot = XCursor::hotspot(frame).to_logical(int_scale);
                let p: Point<f64, Physical> = (local - hotspot.to_f64()).to_physical_precise_round(scale);
                let texture = self.cursor_manager.texture(*icon, int_scale, &cursor, idx);
                match MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    p,
                    &texture,
                    None,
                    None,
                    None,
                    Kind::Cursor,
                ) {
                    Ok(e) => out.push(HecateElement::Memory(e)),
                    Err(err) => tracing::warn!("error importing cursor texture: {err:?}"),
                }
            }
        }
    }

    /// Solid buffers for an output's background and lock screen, sized to the output.
    pub fn update_output_buffers(&mut self, output: &Output) {
        let size = Wm::output_geometry(output).size;
        let color = self.config.general.background_color.premultiplied();
        if let Some(s) = self.output_state.get_mut(output) {
            s.background.update(size, color);
            s.lock_background.update(size, [0., 0., 0., 1.]);
        }
    }
}

/// Make solid-colour buffers for a new output.
pub fn new_output_buffers() -> (SolidColorBuffer, SolidColorBuffer) {
    (SolidColorBuffer::default(), SolidColorBuffer::default())
}

impl Hecate {
    /// Record which output each surface is primarily shown on (for frame callbacks and
    /// dmabuf feedback).
    pub fn update_primary_scanout_output(
        &self,
        output: &Output,
        states: &smithay::backend::renderer::element::RenderElementStates,
    ) {
        use smithay::backend::renderer::element::default_primary_scanout_output_compare;
        use smithay::desktop::utils::{update_surface_primary_scanout_output, with_surfaces_surface_tree};
        for window in self.wm.windows_on_output(output) {
            window.with_surfaces(|surface, data| {
                update_surface_primary_scanout_output(
                    surface,
                    output,
                    data,
                    None,
                    states,
                    default_primary_scanout_output_compare,
                );
            });
        }
        let map = layer_map_for_output(output);
        for layer in map.layers() {
            layer.with_surfaces(|surface, data| {
                update_surface_primary_scanout_output(
                    surface,
                    output,
                    data,
                    None,
                    states,
                    default_primary_scanout_output_compare,
                );
            });
        }
        if let CursorImageStatus::Surface(surface) = &self.cursor_status {
            with_surfaces_surface_tree(surface, |surface, data| {
                update_surface_primary_scanout_output(
                    surface,
                    output,
                    data,
                    None,
                    states,
                    default_primary_scanout_output_compare,
                );
            });
        }
    }

    pub fn take_presentation_feedback(
        &self,
        output: &Output,
        states: &smithay::backend::renderer::element::RenderElementStates,
    ) -> smithay::desktop::utils::OutputPresentationFeedback {
        use smithay::desktop::utils::{
            OutputPresentationFeedback, surface_presentation_feedback_flags_from_states,
            surface_primary_scanout_output,
        };
        let mut feedback = OutputPresentationFeedback::new(output);
        for window in self.wm.windows_on_output(output) {
            window.take_presentation_feedback(&mut feedback, surface_primary_scanout_output, |s, _| {
                surface_presentation_feedback_flags_from_states(s, None, states)
            });
        }
        let map = layer_map_for_output(output);
        for layer in map.layers() {
            layer.take_presentation_feedback(&mut feedback, surface_primary_scanout_output, |s, _| {
                surface_presentation_feedback_flags_from_states(s, None, states)
            });
        }
        feedback
    }
}

impl crate::state::State {
    /// Keep a snapshot of a window about to go away, for its close animation.
    pub fn snapshot_closing_window(&mut self, window: &smithay::desktop::Window) {
        let hecate = &mut self.hecate;
        if window.data().rules.no_animations {
            return;
        }
        let Some(anim) = Animation::start(&hecate.wm.clock, &hecate.wm.anim, &hecate.wm.anim.window_close)
        else {
            return;
        };
        let workspace = window.data().workspace;
        if !hecate.wm.is_workspace_active(workspace) || window.data().minimized {
            return;
        }
        let Some(output) = hecate.wm.output_for_window(window).cloned() else {
            return;
        };
        let scale = Scale::from(output.current_scale().fractional_scale());
        let style = border::style_for(&hecate.config, window, false);
        let geo = hecate.wm.window_rect(window);
        let bw = style.width.ceil() as i32;
        let rect = Rectangle::new(
            (geo.loc.x - bw, geo.loc.y - bw).into(),
            (geo.size.w + 2 * bw, geo.size.h + 2 * bw).into(),
        );
        if rect.size.w <= 0 || rect.size.h <= 0 {
            return;
        }
        let alpha = window.data().rules.opacity.unwrap_or(1.0).clamp(0., 1.);
        let texture = self.backend.with_renderer(|renderer| {
            let mut elems = Vec::new();
            // Lay the window out relative to the snapshot's top-left corner.
            border::push_window(renderer, &mut elems, window, geo, rect.loc, scale, alpha, &style);
            snapshot::render_to_texture(
                renderer,
                rect.size.to_f64().to_physical(scale).to_i32_round(),
                scale,
                &elems,
            )
            .map(|t| (t, smithay::backend::renderer::Renderer::context_id(renderer)))
        });
        let Some(Ok((texture, context))) = texture else {
            return;
        };
        hecate.wm.closing.push(crate::wm::Closing {
            texture,
            id: smithay::backend::renderer::element::Id::new(),
            context,
            rect,
            workspace: Some(workspace),
            layer: None,
            anim,
            slide: Point::default(),
        });
        hecate.queue_redraw(&output);
    }
}

pub mod snapshot {
    use anyhow::Context as _;
    use smithay::backend::allocator::Fourcc;
    use smithay::backend::renderer::damage::OutputDamageTracker;
    use smithay::backend::renderer::element::RenderElement;
    use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
    use smithay::backend::renderer::{Bind, Color32F, Offscreen};
    use smithay::utils::{Physical, Scale, Size, Transform};

    /// Render elements into a new transparent texture of `size`.
    pub fn render_to_texture(
        renderer: &mut GlesRenderer,
        size: Size<i32, Physical>,
        scale: Scale<f64>,
        elements: &[impl RenderElement<GlesRenderer>],
    ) -> anyhow::Result<GlesTexture> {
        let mut texture: GlesTexture = renderer
            .create_buffer(Fourcc::Abgr8888, (size.w, size.h).into())
            .context("error creating texture")?;
        {
            let mut target = renderer.bind(&mut texture).context("error binding texture")?;
            let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
            tracker
                .render_output(renderer, &mut target, 0, elements, Color32F::TRANSPARENT)
                .context("error rendering snapshot")?;
        }
        Ok(texture)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlideEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Hecate {
    /// How a layer surface opens or closes: the animation, and the offset it slides from (open)
    /// or to (close).
    pub fn layer_animation(
        &self,
        layer: &smithay::desktop::LayerSurface,
        output: &Output,
        closing: bool,
    ) -> Option<(Animation, Point<f64, Logical>)> {
        use crate::config::{AnimStyle, LayerAnimation};
        let rules = layer_rules_for(&self.config, layer.namespace());
        let mut anim = if closing {
            self.wm.anim.layer_close.clone()
        } else {
            self.wm.anim.layer_open.clone()
        };
        let mut edge = None;
        match &rules.animation {
            None => {}
            Some(LayerAnimation::Popin { popin }) => {
                anim.style = AnimStyle::Popin;
                anim.from = *popin;
            }
            Some(LayerAnimation::Named(name)) => {
                let (style, e) = match name.as_str() {
                    "none" => return None,
                    "fade" => (AnimStyle::Fade, None),
                    "popin" => (AnimStyle::Popin, None),
                    "slide" => (AnimStyle::Slide, None),
                    "slide_top" => (AnimStyle::Slide, Some(SlideEdge::Top)),
                    "slide_bottom" => (AnimStyle::Slide, Some(SlideEdge::Bottom)),
                    "slide_left" => (AnimStyle::Slide, Some(SlideEdge::Left)),
                    "slide_right" => (AnimStyle::Slide, Some(SlideEdge::Right)),
                    other => {
                        tracing::warn!("unknown layer animation {other:?}");
                        (anim.style, None)
                    }
                };
                anim.style = style;
                edge = e;
            }
        }
        let a = Animation::start(&self.wm.clock, &self.wm.anim, &anim)?;
        if anim.style != AnimStyle::Slide {
            return Some((a, Point::default()));
        }
        let geo = layer_map_for_output(output).layer_geometry(layer)?;
        let size = Wm::output_geometry(output).size;
        // By default, slide from the output edge nearest to the surface.
        let edge = edge.unwrap_or_else(|| {
            let gaps = [
                (geo.loc.y, SlideEdge::Top),
                (size.h - geo.loc.y - geo.size.h, SlideEdge::Bottom),
                (geo.loc.x, SlideEdge::Left),
                (size.w - geo.loc.x - geo.size.w, SlideEdge::Right),
            ];
            gaps.into_iter().min_by_key(|(gap, _)| *gap).unwrap().1
        });
        let offset = match edge {
            SlideEdge::Top => (0, -(geo.loc.y + geo.size.h)),
            SlideEdge::Bottom => (0, size.h - geo.loc.y),
            SlideEdge::Left => (-(geo.loc.x + geo.size.w), 0),
            SlideEdge::Right => (size.w - geo.loc.x, 0),
        };
        Some((a, Point::from(offset).to_f64()))
    }

    /// A layer surface (re)mapped or unmapped: start or reset its open animation.
    pub fn layer_mapped(&mut self, layer: &smithay::desktop::LayerSurface, output: &Output, mapped: bool) {
        let state = layer
            .user_data()
            .get_or_insert(|| std::cell::RefCell::new(crate::anim::LayerAnimState::default()));
        let was_shown = std::mem::replace(&mut state.borrow_mut().shown, mapped);
        if mapped && !was_shown {
            let open = self.layer_animation(layer, output, false);
            state.borrow_mut().open = open;
        } else if !mapped {
            state.borrow_mut().open = None;
        }
    }
}

impl crate::state::State {
    /// Keep a snapshot of a layer surface about to go away, for its close animation.
    pub fn snapshot_closing_layer(&mut self, layer: &smithay::desktop::LayerSurface, output: &Output) {
        let shown = layer
            .user_data()
            .get::<std::cell::RefCell<crate::anim::LayerAnimState>>()
            .is_some_and(|s| s.borrow().shown);
        if !shown || self.hecate.wm.is_locked() {
            return;
        }
        let Some((anim, slide)) = self.hecate.layer_animation(layer, output, true) else {
            return;
        };
        let Some(geo) = layer_map_for_output(output).layer_geometry(layer) else {
            return;
        };
        if geo.size.w <= 0 || geo.size.h <= 0 {
            return;
        }
        let scale = Scale::from(output.current_scale().fractional_scale());
        let alpha = layer_rules_for(&self.hecate.config, layer.namespace())
            .opacity
            .unwrap_or(1.0)
            .clamp(0., 1.);
        let texture = self.backend.with_renderer(|renderer| {
            let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                layer.render_elements(renderer, Point::from((0, 0)), scale, alpha);
            snapshot::render_to_texture(
                renderer,
                geo.size.to_f64().to_physical(scale).to_i32_round(),
                scale,
                &elems,
            )
            .map(|t| (t, smithay::backend::renderer::Renderer::context_id(renderer)))
        });
        let Some(Ok((texture, context))) = texture else {
            return;
        };
        let output_loc = Wm::output_geometry(output).loc;
        self.hecate.wm.closing.push(crate::wm::Closing {
            texture,
            id: smithay::backend::renderer::element::Id::new(),
            context,
            rect: Rectangle::new(geo.loc + output_loc, geo.size),
            workspace: None,
            layer: Some(layer.layer()),
            anim,
            slide,
        });
        self.hecate.queue_redraw(output);
    }
}
