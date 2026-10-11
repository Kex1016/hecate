//! Building the list of render elements for an output.

pub mod border;

use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::output::Output;
use smithay::render_elements;
use smithay::utils::{Logical, Physical, Point, Scale};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;

use crate::config::{Config, LayerRule};
use crate::cursor::XCursor;
use crate::state::Hecate;
use crate::wm::{WindowExt, Wm};

render_elements! {
    pub HecateElement<=GlesRenderer>;
    Surface = WaylandSurfaceRenderElement<GlesRenderer>,
    Solid = SolidColorRenderElement,
    Memory = MemoryRenderBufferRenderElement<GlesRenderer>,
    Border = border::BorderElement,
    Clipped = border::ClippedSurfaceElement,
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
        let mut out: Vec<HecateElement> = Vec::new();
        let scale = Scale::from(output.current_scale().fractional_scale());
        let output_geo = Wm::output_geometry(output);
        let to_local = |p: Point<i32, Logical>| -> Point<i32, Physical> {
            (p - output_geo.loc).to_physical_precise_round(scale)
        };
        let state = self.output_state.get(output);

        // Cursor on top.
        self.push_cursor(renderer, output, &mut out);

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
        let push_layer = |out: &mut Vec<HecateElement>, renderer: &mut GlesRenderer, layer: Layer| {
            for surface in map.layers_on(layer).rev() {
                let Some(geo) = map.layer_geometry(surface) else {
                    continue;
                };
                let rules = layer_rules_for(&self.config, surface.namespace());
                let alpha = rules.opacity.unwrap_or(1.0).clamp(0., 1.);
                let loc = geo.loc.to_physical_precise_round(scale);
                let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    surface.render_elements(renderer, loc, scale, alpha);
                out.extend(elems.into_iter().map(HecateElement::Surface));
            }
        };

        push_layer(&mut out, renderer, Layer::Overlay);

        let windows = self.wm.windows_on_output(output);
        let fullscreen = windows.iter().any(|w| w.data().fullscreen);
        if !fullscreen {
            push_layer(&mut out, renderer, Layer::Top);
        }

        let focused = self
            .seat
            .get_keyboard()
            .and_then(|k| k.current_focus())
            .map(|s| crate::utils::root_surface(&s));
        for window in windows.iter().rev() {
            let render_loc = to_local(Wm::render_location(window));
            let is_focused = focused.as_ref() == Some(window.toplevel_surface().wl_surface());
            let alpha = window.data().rules.opacity.unwrap_or(1.0).clamp(0., 1.);
            let style = if self.identify_until.is_some() {
                border::style_for_identify(&self.config, window, is_focused)
            } else {
                border::style_for(&self.config, window, is_focused)
            };
            let geo_rect = self.wm.window_rect(window);
            let _ = render_loc;
            border::push_window(
                renderer,
                &mut out,
                window,
                geo_rect,
                output_geo.loc,
                scale,
                alpha,
                &style,
            );
        }

        if fullscreen {
            push_layer(&mut out, renderer, Layer::Top);
        }
        push_layer(&mut out, renderer, Layer::Bottom);
        push_layer(&mut out, renderer, Layer::Background);
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
        out
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
