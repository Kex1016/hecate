//! Protocol handlers.

mod compositor;
mod layer_shell;
mod xdg_shell;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::input::TabletToolDescriptor;
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::dnd::{DnDGrab, DndGrabHandler, DndTarget, GrabType, Source};
use smithay::input::keyboard::LedState;
use smithay::input::pointer::{CursorImageStatus, Focus, PointerHandle};
use smithay::input::tablet::TabletSeatHandler;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as XdgDecoMode;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Serial};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::wayland::fractional_scale::{FractionalScaleHandler, with_fractional_scale};
use smithay::wayland::idle_inhibit::IdleInhibitHandler;
use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};
use smithay::wayland::input_method::{InputMethodHandler, PopupSurface};
use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::pointer_constraints::{
    ConstraintRemove, PointerConstraintsHandler, with_pointer_constraint,
};
use smithay::wayland::security_context::{
    SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler, set_data_device_focus,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState, set_primary_focus,
};
use smithay::wayland::session_lock::{
    LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
};
use smithay::wayland::shell::kde::decoration::{KdeDecorationHandler, KdeDecorationState};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::text_input::TextInputActivation;
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};

use crate::config::DecorationMode;
use crate::state::{ClientState, State};
use crate::wm::WindowExt;

smithay::delegate_dispatch2!(State);

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.hecate.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.hecate.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.hecate.cursor_status = image;
        self.hecate.queue_redraw_all();
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        self.backend.update_led_state(led_state);
    }
}

impl TabletSeatHandler for State {
    type ToolFocus = WlSurface;

    fn tablet_tool_image(&mut self, _tool: &TabletToolDescriptor, image: CursorImageStatus) {
        self.hecate.cursor_status = image;
        self.hecate.queue_redraw_all();
    }
}

impl DataDeviceHandler for State {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.hecate.data_device_state
    }
}

impl WaylandDndGrabHandler for State {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        self.hecate.dnd_icon = icon;
        match type_ {
            GrabType::Pointer => {
                let pointer = seat.get_pointer().unwrap();
                if let Some(start_data) = pointer.grab_start_data() {
                    let grab = DnDGrab::new_pointer(&self.hecate.display_handle, start_data, source, seat);
                    pointer.set_grab(self, grab, serial, Focus::Keep);
                }
            }
            GrabType::Touch => {
                if let Some(touch) = seat.get_touch()
                    && let Some(start_data) = touch.grab_start_data()
                {
                    let grab = DnDGrab::new_touch(&self.hecate.display_handle, start_data, source, seat);
                    touch.set_grab(self, grab, serial);
                }
            }
        }
    }
}

impl DndGrabHandler for State {
    fn dropped(
        &mut self,
        _target: Option<DndTarget<'_, Self>>,
        _validated: bool,
        _seat: Seat<Self>,
        _location: Point<f64, Logical>,
    ) {
        self.hecate.dnd_icon = None;
        self.hecate.queue_redraw_all();
    }
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.hecate.primary_selection_state
    }
}

impl smithay::wayland::selection::wlr_data_control::DataControlHandler for State {
    fn data_control_state(&mut self) -> &mut smithay::wayland::selection::wlr_data_control::DataControlState {
        &mut self.hecate.wlr_data_control_state
    }
}

impl smithay::wayland::selection::ext_data_control::DataControlHandler for State {
    fn data_control_state(&mut self) -> &mut smithay::wayland::selection::ext_data_control::DataControlState {
        &mut self.hecate.ext_data_control_state
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.hecate.shm_state
    }
}

impl OutputHandler for State {
    fn output_bound(&mut self, output: Output, wl_output: WlOutput) {
        crate::protocols::foreign_toplevel::on_output_bound(self, &output, &wl_output);
        crate::protocols::ext_workspace::on_output_bound(self, &output, &wl_output);
    }
}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.hecate.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        if self.backend.import_dmabuf(&dmabuf) {
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

impl FractionalScaleHandler for State {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        // Use the scale of the output the surface's root is on, else the first output.
        let root = crate::utils::root_surface(&surface);
        let output = self
            .hecate
            .window_for_surface(&root)
            .and_then(|w| self.hecate.wm.output_for_window(&w).cloned())
            .or_else(|| self.hecate.wm.outputs().next().cloned());
        if let Some(output) = output {
            let scale = output.current_scale().fractional_scale();
            smithay::wayland::compositor::with_states(&surface, |states| {
                with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
            });
        }
    }
}

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.hecate.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        // Only grant tokens tied to a recent input event of the focused client.
        let Some((serial, seat)) = data.serial else {
            return false;
        };
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        Seat::from_resource(&seat).as_ref() == Some(&self.hecate.seat)
            && keyboard
                .last_enter()
                .is_some_and(|last| serial.is_no_older_than(&last))
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        let Some(window) = self.hecate.window_for_surface(&surface) else {
            return;
        };
        if window.data().rules.ignore_focus_requests {
            return;
        }
        let allowed = match self.hecate.config.focus.steal {
            crate::config::FocusSteal::Never => false,
            crate::config::FocusSteal::Always => true,
            crate::config::FocusSteal::Smart => token_data.timestamp.elapsed().as_secs() < 10,
        };
        if allowed {
            self.activate_window(&window);
        } else {
            window.data().urgent = true;
            self.hecate.queue_redraw_all();
        }
    }
}

impl XdgForeignHandler for State {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.hecate.xdg_foreign_state
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        let mode = self.decoration_mode_for(&toplevel);
        toplevel.with_pending_state(|state| state.decoration_mode = Some(mode));
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: XdgDecoMode) {
        // We decide; clients get told what we picked.
        let mode = self.decoration_mode_for(&toplevel);
        toplevel.with_pending_state(|state| state.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        let mode = self.decoration_mode_for(&toplevel);
        toplevel.with_pending_state(|state| state.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
}

impl KdeDecorationHandler for State {
    fn kde_decoration_state(&self) -> &KdeDecorationState {
        &self.hecate.kde_decoration_state
    }
}

impl State {
    /// Server-side unless the config or a window rule asks for client-side.
    pub fn decoration_mode_for(&self, toplevel: &ToplevelSurface) -> XdgDecoMode {
        let rule = self
            .hecate
            .window_for_surface(toplevel.wl_surface())
            .and_then(|w| w.data().rules.decorations);
        match rule.unwrap_or(self.hecate.config.general.decorations) {
            DecorationMode::Server => XdgDecoMode::ServerSide,
            DecorationMode::Client => XdgDecoMode::ClientSide,
        }
    }
}

impl IdleNotifierHandler for State {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.hecate.idle_notifier_state
    }
}

impl IdleInhibitHandler for State {
    fn inhibit(&mut self, surface: WlSurface) {
        self.hecate.idle_inhibitors.push(surface);
        self.refresh_idle_inhibit();
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        self.hecate.idle_inhibitors.retain(|s| s != &surface);
        self.refresh_idle_inhibit();
    }
}

impl State {
    pub fn refresh_idle_inhibit(&mut self) {
        self.hecate.idle_inhibitors.retain(|s| s.is_alive());
        let fullscreen = self.hecate.config.gaming.fullscreen_inhibits_idle
            && self
                .hecate
                .wm
                .visible_windows()
                .iter()
                .any(|w| w.data().fullscreen);
        let inhibited = fullscreen || !self.hecate.idle_inhibitors.is_empty();
        self.hecate.idle_notifier_state.set_is_inhibited(inhibited);
    }
}

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.hecate.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        inhibitor.activate();
        self.hecate
            .shortcut_inhibitors
            .push(inhibitor.wl_surface().clone());
    }

    fn inhibitor_destroyed(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        let surface = inhibitor.wl_surface().clone();
        self.hecate.shortcut_inhibitors.retain(|s| s != &surface);
    }
}

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        if pointer.current_focus().as_ref() == Some(surface) {
            with_pointer_constraint(surface, pointer, |constraint| {
                if let Some(c) = constraint {
                    c.activate();
                }
            });
        }
    }

    fn cursor_position_hint(
        &mut self,
        surface: &WlSurface,
        pointer: &PointerHandle<Self>,
        location: Point<f64, Logical>,
    ) {
        let active = with_pointer_constraint(surface, pointer, |c| c.is_some_and(|c| c.is_active()));
        if !active {
            return;
        }
        let root = crate::utils::root_surface(surface);
        if let Some(window) = self.hecate.window_for_surface(&root) {
            let origin = crate::wm::Wm::render_location(&window).to_f64();
            let pos = origin + location;
            pointer.set_location(pos);
            self.hecate.pointer_location = pos;
        }
    }

    fn remove_constraint(
        &mut self,
        _surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        _remove: ConstraintRemove,
    ) {
    }
}

impl SecurityContextHandler for State {
    fn context_created(&mut self, source: SecurityContextListenerSource, context: SecurityContext) {
        let res = self
            .hecate
            .event_loop
            .insert_source(source, move |client, _, state| {
                let data = ClientState {
                    security_context: Some(context.clone()),
                    restricted: true,
                    ..Default::default()
                };
                if let Err(err) = state
                    .hecate
                    .display_handle
                    .insert_client(client, std::sync::Arc::new(data))
                {
                    tracing::warn!("error adding sandboxed client: {err}");
                }
            });
        if let Err(err) = res {
            tracing::warn!("error inserting security context listener: {err}");
        }
    }
}

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.hecate.popups.track_popup(PopupKind::from(surface)) {
            tracing::warn!("error tracking input method popup: {err}");
        }
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {}

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|p| p.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.hecate
            .window_for_surface(parent)
            .map(|w| w.geometry())
            .unwrap_or_default()
    }
}

impl TextInputActivation for State {}

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.hecate.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        self.hecate.wm.lock = crate::wm::LockState::Locked;
        // We render nothing but lock surfaces (or black) from now on, so confirm right away.
        confirmation.lock();
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, None, smithay::utils::SERIAL_COUNTER.next_serial());
        self.hecate.queue_redraw_all();
    }

    fn unlock(&mut self) {
        self.hecate.wm.unlock();
        self.focus_topmost();
        self.hecate.queue_redraw_all();
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        let Some(output) = smithay::output::Output::from_resource(&wl_output) else {
            return;
        };
        let geo = crate::wm::Wm::output_geometry(&output);
        surface.with_pending_state(|state| {
            state.size = Some((geo.size.w as u32, geo.size.h as u32).into());
        });
        surface.send_configure();
        let wl = surface.wl_surface().clone();
        self.hecate.wm.set_lock_surface(&output, surface);
        if self.hecate.wm.focused_output.as_ref() == Some(&output) || self.hecate.wm.outputs().count() == 1 {
            let keyboard = self.hecate.seat.get_keyboard().unwrap();
            keyboard.set_focus(self, Some(wl), smithay::utils::SERIAL_COUNTER.next_serial());
        }
        self.hecate.queue_redraw(&output);
    }
}
