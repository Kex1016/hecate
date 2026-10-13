use smithay::desktop::{
    PopupKeyboardGrab, PopupKind, PopupManager, PopupPointerGrab, PopupUngrabStrategy, Window,
    WindowSurfaceType, find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output,
};
use smithay::input::pointer::Focus;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::utils::{Rectangle, Serial};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::state::State;
use crate::wm::window::ResizeEdge;
use crate::wm::{WindowExt, Wm};

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.hecate.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        // Touch the data so the id is assigned in creation order.
        let _ = window.wid();
        self.hecate.wm.pending.push(window);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let popup = PopupKind::Xdg(surface);
        self.unconstrain_popup(&popup);
        if let Err(err) = self.hecate.popups.track_popup(popup) {
            tracing::warn!("error tracking popup: {err}");
        }
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&PopupKind::Xdg(surface.clone()));
        surface.send_repositioned(token);
    }

    fn grab(&mut self, surface: PopupSurface, _seat: WlSeat, serial: Serial) {
        let popup = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&popup) else {
            return;
        };
        let seat = self.hecate.seat.clone();
        let mut grab = match self.hecate.popups.grab_popup(root.clone(), popup, &seat, serial) {
            Ok(grab) => grab,
            Err(err) => {
                tracing::trace!("ignoring popup grab: {err:?}");
                return;
            }
        };
        let keyboard = seat.get_keyboard().unwrap();
        let pointer = seat.get_pointer().unwrap();
        let can_focus = self
            .hecate
            .wm
            .outputs()
            .find_map(|o| {
                layer_map_for_output(o)
                    .layer_for_surface(&root, WindowSurfaceType::TOPLEVEL)
                    .map(|l| l.can_receive_keyboard_focus())
            })
            .unwrap_or(true);
        let kb_mismatch = keyboard.is_grabbed()
            && !(keyboard.has_grab(serial) || grab.previous_serial().is_none_or(|s| keyboard.has_grab(s)));
        let ptr_mismatch = pointer.is_grabbed()
            && !(pointer.has_grab(serial) || grab.previous_serial().is_none_or(|s| pointer.has_grab(s)));
        if (can_focus && kb_mismatch) || ptr_mismatch {
            grab.ungrab(PopupUngrabStrategy::All);
            return;
        }
        if can_focus {
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, serial: Serial) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.start_interactive_move(&window, Some(serial));
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        _seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.start_interactive_resize(&window, ResizeEdge::from(edges), Some(serial));
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.set_maximized(&window, crate::wm::Maximized::Both);
        } else {
            surface.with_pending_state(|s| s.states.set(xdg_toplevel::State::Maximized));
        }
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.set_maximized(&window, crate::wm::Maximized::No);
        } else {
            surface.with_pending_state(|s| s.states.unset(xdg_toplevel::State::Maximized));
        }
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, output: Option<WlOutput>) {
        let output = output.and_then(|o| smithay::output::Output::from_resource(&o));
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.set_fullscreen(&window, true, output);
        } else {
            surface.with_pending_state(|s| s.states.set(xdg_toplevel::State::Fullscreen));
        }
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.set_fullscreen(&window, false, None);
        } else {
            surface.with_pending_state(|s| {
                s.states.unset(xdg_toplevel::State::Fullscreen);
                s.fullscreen_output = None;
            });
        }
        if surface.is_initial_configure_sent() {
            surface.send_pending_configure();
        }
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.set_minimized(&window, true);
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            let output = self.hecate.wm.output_for_window(&window).cloned();
            self.snapshot_closing_window(&window);
            self.hecate.wm.remove_window(&window);
            self.on_window_closed(&window);
            if let Some(o) = output {
                self.hecate.queue_redraw(&o);
            }
            self.focus_topmost();
        }
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.apply_window_rules(&window, false);
            self.on_window_changed(&window);
        }
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.hecate.window_for_surface(surface.wl_surface()) {
            self.apply_window_rules(&window, false);
            self.on_window_changed(&window);
        }
    }
}

impl State {
    pub fn unconstrain_popup(&self, popup: &PopupKind) {
        let Ok(root) = find_popup_root_surface(popup) else {
            return;
        };
        if let Some(window) = self.hecate.window_for_surface(&root) {
            let output = self
                .hecate
                .wm
                .output_for_window(&window)
                .cloned()
                .or_else(|| self.hecate.wm.outputs().next().cloned());
            let Some(output) = output else {
                return;
            };
            // Target rect relative to the popup parent's geometry.
            let mut target = Wm::output_geometry(&output);
            target.loc -= window.data().loc;
            target.loc -= get_popup_toplevel_coords(popup);
            if let PopupKind::Xdg(p) = popup {
                p.with_pending_state(|state| {
                    state.geometry = state.positioner.get_unconstrained_geometry(target);
                });
            }
            return;
        }
        for output in self.hecate.wm.outputs() {
            let map = layer_map_for_output(output);
            let Some(layer) = map.layer_for_surface(&root, WindowSurfaceType::TOPLEVEL) else {
                continue;
            };
            let Some(layer_geo) = map.layer_geometry(layer) else {
                return;
            };
            let out_geo = Wm::output_geometry(output);
            let mut target = Rectangle::from_size(out_geo.size);
            target.loc -= layer_geo.loc;
            target.loc -= get_popup_toplevel_coords(popup);
            if let PopupKind::Xdg(p) = popup {
                p.with_pending_state(|state| {
                    state.geometry = state.positioner.get_unconstrained_geometry(target);
                });
            }
            return;
        }
    }

    /// Dismiss all popups of a surface (used when it loses focus or closes).
    pub fn dismiss_popups(
        &mut self,
        root: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
        for (popup, _) in PopupManager::popups_for_surface(root) {
            let _ = PopupManager::dismiss_popup(root, &popup);
        }
    }
}
