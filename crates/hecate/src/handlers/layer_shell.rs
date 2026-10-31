use smithay::desktop::{LayerSurface, PopupKind, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::{
    self, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::PopupSurface;

use crate::state::State;

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.hecate.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| self.hecate.wm.focused_output.clone())
            .or_else(|| self.hecate.wm.outputs().next().cloned());
        let Some(output) = output else {
            tracing::warn!("no output for new layer surface {namespace:?}, closing it");
            surface.send_close();
            return;
        };
        let mut map = layer_map_for_output(&output);
        if let Err(err) = map.map_layer(&LayerSurface::new(surface, namespace)) {
            tracing::warn!("error mapping layer surface: {err}");
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let found = self.hecate.wm.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map.layers().find(|l| l.layer_surface() == &surface).cloned();
            layer.map(|l| (o.clone(), l))
        });
        if let Some((output, layer)) = found {
            self.snapshot_closing_layer(&layer, &output);
            layer_map_for_output(&output).unmap_layer(&layer);
            self.on_usable_area_changed(&output);
            self.focus_topmost();
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        self.unconstrain_popup(&PopupKind::Xdg(popup));
    }
}

impl State {
    /// Handle a commit on a layer surface root. Returns false if `root` isn't one.
    pub fn layer_surface_commit(&mut self, root: &WlSurface) -> bool {
        let output = self
            .hecate
            .wm
            .outputs()
            .find(|o| {
                layer_map_for_output(o)
                    .layer_for_surface(root, WindowSurfaceType::TOPLEVEL)
                    .is_some()
            })
            .cloned();
        let Some(output) = output else {
            return false;
        };

        let mut map = layer_map_for_output(&output);
        let old_zone = map.non_exclusive_zone();
        map.arrange();
        let new_zone = map.non_exclusive_zone();
        let layer = map.layer_for_surface(root, WindowSurfaceType::TOPLEVEL).cloned();
        drop(map);

        if let Some(layer) = layer {
            let initial_sent = with_states(root, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .map(|d| d.lock().unwrap().initial_configure_sent)
                    .unwrap_or(true)
            });
            let keyboard = self.hecate.seat.get_keyboard().unwrap();
            if initial_sent {
                self.hecate
                    .layer_mapped(&layer, &output, crate::utils::is_mapped(root));
            }
            if !crate::utils::is_mapped(root) && keyboard.current_focus().as_ref() == Some(root) {
                // Unmapped with a null buffer: give the keyboard back.
                self.focus_topmost();
            }
            if !initial_sent {
                let scale = output.current_scale();
                let transform = output.current_transform();
                with_states(root, |states| {
                    crate::utils::send_scale_transform(root, states, scale, transform);
                });
                layer.layer_surface().send_configure();
            } else if crate::utils::is_mapped(root) {
                let interactivity = layer.cached_state().keyboard_interactivity;
                let can_focus = interactivity != wlr_layer::KeyboardInteractivity::None
                    && !self.layer_rule_no_focus(&layer);
                let seen = layer.user_data().get::<LayerSeen>().is_some();
                // On the first map only, or when no exclusive layer holds the keyboard: not on
                // every commit, or two exclusive surfaces (the greeter's lock screen has one per
                // output, redrawing every frame) would trade the keyboard back and forth, with a
                // key's press going to one and its release to the other.
                let is_newly_focusable = match interactivity {
                    wlr_layer::KeyboardInteractivity::Exclusive => {
                        !seen || !self.keyboard_on_exclusive_layer()
                    }
                    wlr_layer::KeyboardInteractivity::OnDemand => !seen,
                    wlr_layer::KeyboardInteractivity::None => false,
                };
                layer.user_data().insert_if_missing(|| LayerSeen);
                // Layers drawn under a fullscreen window (everything but Overlay: the bar,
                // its popups, notifications) don't take the keyboard from it, or a
                // notification mid-game sends the game's input to the shell.
                let under_fullscreen = layer.layer() != wlr_layer::Layer::Overlay
                    && self.fullscreen_window_on(&output).is_some();
                if can_focus && is_newly_focusable && !under_fullscreen && !self.hecate.wm.is_locked() {
                    let keyboard = self.hecate.seat.get_keyboard().unwrap();
                    keyboard.set_focus(
                        self,
                        Some(root.clone()),
                        smithay::utils::SERIAL_COUNTER.next_serial(),
                    );
                }
            }
        }

        if old_zone != new_zone {
            self.on_usable_area_changed(&output);
        }
        self.hecate.queue_redraw(&output);
        true
    }

    /// Does a mapped layer surface with exclusive keyboard interactivity have the keyboard?
    fn keyboard_on_exclusive_layer(&self) -> bool {
        let Some(focus) = self.hecate.seat.get_keyboard().and_then(|k| k.current_focus()) else {
            return false;
        };
        self.hecate.wm.outputs().any(|o| {
            layer_map_for_output(o)
                .layer_for_surface(&focus, WindowSurfaceType::TOPLEVEL)
                .is_some_and(|l| {
                    l.cached_state().keyboard_interactivity == wlr_layer::KeyboardInteractivity::Exclusive
                        && crate::utils::is_mapped(l.wl_surface())
                })
        })
    }

    fn layer_rule_no_focus(&self, layer: &LayerSurface) -> bool {
        crate::render::layer_rules_for(&self.hecate.config, layer.namespace())
            .no_keyboard_focus
            .unwrap_or(false)
    }
}

/// Marker: the layer surface has been mapped before (for on-demand focus on first map only).
struct LayerSeen;
