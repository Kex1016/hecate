//! Global compositor state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::desktop::{PopupManager, Window};
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::WmCapabilities;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Display, DisplayHandle};
use smithay::utils::{ClockSource, Logical, Monotonic, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::dmabuf::DmabufState;
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::idle_inhibit::IdleInhibitManagerState;
use smithay::wayland::idle_notify::IdleNotifierState;
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::security_context::{SecurityContext, SecurityContextState};
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::ext_data_control::DataControlState as ExtDataControlState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState as WlrDataControlState;
use smithay::wayland::session_lock::SessionLockManagerState;
use smithay::wayland::shell::kde::decoration::KdeDecorationState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::xdg_foreign::XdgForeignState;

use crate::backend::Backend;
use crate::config::keys::{KeyCombo, Modifiers};
use crate::config::{Bind, Config, MouseBind};
use crate::cursor::CursorManager;
use crate::wm::Wm;

pub struct State {
    pub backend: Backend,
    pub hecate: Hecate,
}

/// Per-output render bookkeeping.
#[derive(Debug, Default)]
pub struct OutputState {
    pub redraw_queued: bool,
    /// A frame was submitted and we are waiting for the backend to present it.
    pub waiting_for_vblank: bool,
    pub frame_callback_sequence: u32,
    pub animations_running: bool,
    pub background: smithay::backend::renderer::element::solid::SolidColorBuffer,
    pub lock_background: smithay::backend::renderer::element::solid::SolidColorBuffer,
    pub global: Option<smithay::reexports::wayland_server::backend::GlobalId>,
    /// Powered off via DPMS (action or wlr-output-power).
    pub powered_off: bool,
    /// VRR state last sent to the backend.
    pub vrr_applied: Option<bool>,
    pub frame_stats: crate::frame_stats::FrameStats,
}

pub struct Hecate {
    /// Effective config: the file plus runtime overrides.
    pub config: Config,
    /// The config as loaded from disk.
    pub file_config: Config,
    /// Runtime overrides (`hecate msg set`), in order.
    pub overrides: Vec<(String, serde_json::Value)>,
    pub config_reload_failures: u64,
    pub last_config_error: Option<String>,
    pub ipc: Option<crate::ipc::IpcServer>,
    /// Windows highlighted by `hecate msg identify` until this time.
    pub identify_until: Option<Instant>,
    pub config_path: Option<PathBuf>,
    pub binds: Vec<Bind>,
    pub mouse_binds: Vec<(KeyCombo, MouseBind)>,
    /// `MOD` resolves to this.
    pub mod_key: Modifiers,
    pub keybinds_enabled: bool,

    pub event_loop: LoopHandle<'static, State>,
    pub stop_signal: LoopSignal,
    pub display_handle: DisplayHandle,
    pub socket_name: Option<String>,
    pub start_time: Instant,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub kde_decoration_state: KdeDecorationState,
    pub layer_shell_state: WlrLayerShellState,
    pub session_lock_state: SessionLockManagerState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub dmabuf_state: DmabufState,
    pub seat_state: SeatState<State>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub wlr_data_control_state: WlrDataControlState,
    pub ext_data_control_state: ExtDataControlState,
    pub viewporter_state: ViewporterState,
    pub fractional_scale_state: FractionalScaleManagerState,
    pub presentation_state: PresentationState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_foreign_state: XdgForeignState,
    pub foreign_toplevel_state: crate::protocols::foreign_toplevel::ForeignToplevelManagerState,
    pub ext_workspace_state: crate::protocols::ext_workspace::ExtWorkspaceManagerState,
    pub gamma_control_state: crate::protocols::gamma_control::GammaControlManagerState,
    pub output_management_state: crate::protocols::output_management::OutputManagementState,
    pub output_power_state: crate::protocols::output_power::OutputPowerState,
    pub screencopy_state: crate::protocols::screencopy::ScreencopyManagerState,
    pub focus_grab_state: crate::protocols::focus_grab::FocusGrabState,
    pub tearing_control_state: crate::protocols::tearing_control::TearingControlState,
    pub content_type_state: smithay::wayland::content_type::ContentTypeState,
    pub fifo_state: smithay::wayland::fifo::FifoManagerState,
    pub commit_timing_state: smithay::wayland::commit_timing::CommitTimingManagerState,
    /// A timer for the next commit-timing deadline is pending.
    pub commit_timer_armed: bool,
    pub image_capture_source_state: smithay::wayland::image_capture_source::ImageCaptureSourceState,
    pub output_capture_source_state: smithay::wayland::image_capture_source::OutputCaptureSourceState,
    pub image_copy_capture_state: smithay::wayland::image_copy_capture::ImageCopyCaptureState,
    pub image_copy_sessions: Vec<crate::handlers::image_copy_capture::ImageCopySession>,
    pub cursor_shape_state: CursorShapeManagerState,
    pub single_pixel_buffer_state: SinglePixelBufferState,
    pub idle_notifier_state: IdleNotifierState<State>,
    pub idle_inhibit_state: IdleInhibitManagerState,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub pointer_constraints_state: PointerConstraintsState,
    pub relative_pointer_state: RelativePointerManagerState,
    pub pointer_gestures_state: PointerGesturesState,
    pub security_context_state: SecurityContextState,
    pub text_input_state: TextInputManagerState,
    pub input_method_state: InputMethodManagerState,
    pub virtual_keyboard_state: VirtualKeyboardManagerState,
    pub tablet_state: TabletManagerState,

    pub seat: Seat<State>,
    pub popups: PopupManager,
    pub wm: Wm,
    pub output_state: HashMap<Output, OutputState>,
    /// Outputs changed; output-management needs to resend heads.
    pub output_heads_dirty: bool,
    pub satellite: Option<crate::xwayland::Satellite>,
    /// Outputs were powered off by the action; the next input powers them on.
    pub wake_outputs_on_input: bool,

    pub pointer_location: Point<f64, Logical>,
    pub cursor_status: CursorImageStatus,
    pub cursor_manager: CursorManager,
    pub dnd_icon: Option<WlSurface>,

    /// Surfaces currently inhibiting idle.
    pub idle_inhibitors: Vec<WlSurface>,
    /// Surfaces with an active keyboard shortcuts inhibitor.
    pub shortcut_inhibitors: Vec<WlSurface>,
    /// Keys whose press triggered a bind; their release is swallowed.
    pub suppressed_keys: Vec<u32>,
    /// Buttons whose press triggered a mouse bind.
    pub suppressed_buttons: Vec<u32>,
    /// A modifier went down alone; fire tap binds on release unless something else happens.
    pub pending_mod_tap: Option<Modifiers>,
    /// A config reload is scheduled (debounce).
    pub reload_pending: bool,
    pub config_watcher: Option<crate::config::watch::ConfigWatcher>,
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub security_context: Option<SecurityContext>,
    /// Clients in a security context (flatpak) can't see privileged globals.
    pub restricted: bool,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub fn client_is_unrestricted(client: &Client) -> bool {
    client
        .get_data::<ClientState>()
        .is_none_or(|data| !data.restricted)
}

impl State {
    pub fn new(
        config: Config,
        config_path: Option<PathBuf>,
        event_loop: LoopHandle<'static, State>,
        stop_signal: LoopSignal,
        display: Display<State>,
        backend: Backend,
    ) -> anyhow::Result<Self> {
        Self::new_with_socket(
            config,
            config_path,
            event_loop,
            stop_signal,
            display,
            backend,
            true,
        )
    }

    /// Like `new`, optionally without creating a listening socket (tests insert clients directly).
    pub fn new_with_socket(
        config: Config,
        config_path: Option<PathBuf>,
        event_loop: LoopHandle<'static, State>,
        stop_signal: LoopSignal,
        display: Display<State>,
        backend: Backend,
        create_socket: bool,
    ) -> anyhow::Result<Self> {
        let hecate = Hecate::new(
            config,
            config_path,
            event_loop,
            stop_signal,
            display,
            &backend,
            create_socket,
        )?;
        let mut state = State { backend, hecate };
        state.backend.init(&mut state.hecate);
        state.hecate.wm.ensure_workspaces(&state.hecate.config);
        Ok(state)
    }

    /// Called once per event loop iteration, after dispatching events.
    pub fn refresh_and_flush_clients(&mut self) {
        self.hecate.popups.cleanup();
        self.hecate.wm.refresh();
        self.update_keyboard_focus_if_needed();
        if self.hecate.identify_until.is_some_and(|t| Instant::now() >= t) {
            self.hecate.identify_until = None;
            self.hecate.queue_redraw_all();
        }
        self.redraw_queued_outputs();
        crate::protocols::foreign_toplevel::refresh(self);
        crate::protocols::ext_workspace::refresh(self);
        crate::protocols::output_management::refresh(self);
        self.refresh_image_copy_capture();
        self.refresh_focus_grab();
        self.refresh_vrr();
        self.refresh_commit_timers();
        self.ipc_refresh();
        self.ipc_flush();
        if let Err(err) = self.hecate.display_handle.flush_clients() {
            tracing::warn!("error flushing clients: {err}");
        }
    }

    /// Redraw queued outputs, releasing commit-timing and fifo barriers around each frame.
    pub fn redraw_queued_outputs(&mut self) {
        for output in self.hecate.queued_outputs() {
            self.pre_repaint(&output);
            self.hecate.redraw(&mut self.backend, &output);
            self.post_repaint(&output);
        }
    }

    fn update_keyboard_focus_if_needed(&mut self) {
        // Drop focus from dead surfaces.
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        if let Some(focus) = keyboard.current_focus()
            && !smithay::utils::IsAlive::alive(&focus)
        {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            keyboard.set_focus(self, None, serial);
            self.focus_topmost();
        }
    }
}

impl Hecate {
    fn new(
        config: Config,
        config_path: Option<PathBuf>,
        event_loop: LoopHandle<'static, State>,
        stop_signal: LoopSignal,
        display: Display<State>,
        backend: &Backend,
        create_socket: bool,
    ) -> anyhow::Result<Self> {
        let dh = display.handle();

        let compositor_state = CompositorState::new_v6::<State>(&dh);
        let xdg_shell_state = XdgShellState::new_with_capabilities::<State>(
            &dh,
            [
                WmCapabilities::Fullscreen,
                WmCapabilities::Maximize,
                WmCapabilities::Minimize,
            ],
        );
        let xdg_decoration_state = XdgDecorationState::new::<State>(&dh);
        let kde_decoration_state = KdeDecorationState::new::<State>(
            &dh,
            match config.general.decorations {
                crate::config::DecorationMode::Server => {
                    smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration_manager::Mode::Server
                }
                crate::config::DecorationMode::Client => {
                    smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration_manager::Mode::Client
                }
            },
        );
        let layer_shell_state = WlrLayerShellState::new_with_filter::<State, _>(&dh, client_is_unrestricted);
        let session_lock_state = SessionLockManagerState::new::<State, _>(&dh, client_is_unrestricted);
        let shm_state = ShmState::new::<State>(&dh, vec![wl_shm::Format::Xbgr8888, wl_shm::Format::Abgr8888]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<State>(&dh);
        let dmabuf_state = DmabufState::new();
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<State>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<State>(&dh);
        let wlr_data_control_state =
            WlrDataControlState::new::<State, _>(&dh, Some(&primary_selection_state), client_is_unrestricted);
        let ext_data_control_state =
            ExtDataControlState::new::<State, _>(&dh, Some(&primary_selection_state), client_is_unrestricted);
        let viewporter_state = ViewporterState::new::<State>(&dh);
        let fractional_scale_state = FractionalScaleManagerState::new::<State>(&dh);
        let presentation_state = PresentationState::new::<State>(&dh, Monotonic::ID as u32);
        let xdg_activation_state = XdgActivationState::new::<State>(&dh);
        let xdg_foreign_state = XdgForeignState::new::<State>(&dh);
        let foreign_toplevel_state =
            crate::protocols::foreign_toplevel::ForeignToplevelManagerState::new(&dh, client_is_unrestricted);
        let ext_workspace_state =
            crate::protocols::ext_workspace::ExtWorkspaceManagerState::new(&dh, client_is_unrestricted);
        let gamma_control_state =
            crate::protocols::gamma_control::GammaControlManagerState::new(&dh, client_is_unrestricted);
        let output_management_state =
            crate::protocols::output_management::OutputManagementState::new(&dh, client_is_unrestricted);
        let output_power_state =
            crate::protocols::output_power::OutputPowerState::new(&dh, client_is_unrestricted);
        let screencopy_state =
            crate::protocols::screencopy::ScreencopyManagerState::new(&dh, client_is_unrestricted);
        let focus_grab_state = crate::protocols::focus_grab::FocusGrabState::new(&dh, client_is_unrestricted);
        let tearing_control_state = crate::protocols::tearing_control::TearingControlState::new(&dh);
        let content_type_state = smithay::wayland::content_type::ContentTypeState::new::<State>(&dh);
        let fifo_state = smithay::wayland::fifo::FifoManagerState::new::<State>(&dh);
        let commit_timing_state =
            smithay::wayland::commit_timing::CommitTimingManagerState::new::<State>(&dh);
        let image_capture_source_state =
            smithay::wayland::image_capture_source::ImageCaptureSourceState::new();
        let output_capture_source_state =
            smithay::wayland::image_capture_source::OutputCaptureSourceState::new_with_filter::<State, _>(
                &dh,
                client_is_unrestricted,
            );
        let image_copy_capture_state =
            smithay::wayland::image_copy_capture::ImageCopyCaptureState::new_with_filter::<State, _>(
                &dh,
                client_is_unrestricted,
            );
        dh.create_global::<State, smithay::reexports::wayland_protocols::ext::image_capture_source::v1::server::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1, _>(
            1,
            crate::handlers::image_copy_capture::ToplevelCaptureSourceGlobalData {
                filter: Box::new(client_is_unrestricted),
            },
        );
        let cursor_shape_state = CursorShapeManagerState::new::<State>(&dh);
        let single_pixel_buffer_state = SinglePixelBufferState::new::<State>(&dh);
        let idle_notifier_state = IdleNotifierState::new(&dh, event_loop.clone());
        let idle_inhibit_state = IdleInhibitManagerState::new::<State>(&dh);
        let keyboard_shortcuts_inhibit_state = KeyboardShortcutsInhibitState::new::<State>(&dh);
        let pointer_constraints_state = PointerConstraintsState::new::<State>(&dh);
        let relative_pointer_state = RelativePointerManagerState::new::<State>(&dh);
        let pointer_gestures_state = PointerGesturesState::new::<State>(&dh);
        let security_context_state = SecurityContextState::new::<State, _>(&dh, client_is_unrestricted);
        let text_input_state = TextInputManagerState::new::<State>(&dh);
        let input_method_state = InputMethodManagerState::new::<State, _>(&dh, client_is_unrestricted);
        let virtual_keyboard_state =
            VirtualKeyboardManagerState::new::<State, _>(&dh, client_is_unrestricted);
        let tablet_state = TabletManagerState::new::<State>(&dh);

        let mut seat: Seat<State> = seat_state.new_wl_seat(&dh, backend.seat_name());
        let kb = &config.keyboard;
        let xkb = XkbConfig {
            rules: &kb.rules,
            model: &kb.model,
            layout: &kb.layout,
            variant: &kb.variant,
            options: if kb.options.is_empty() {
                None
            } else {
                Some(kb.options.clone())
            },
        };
        let keyboard = match seat.add_keyboard(xkb, kb.repeat_delay, kb.repeat_rate) {
            Ok(k) => k,
            Err(err) => {
                tracing::warn!("error loading the configured keymap ({err:?}), using the default");
                seat.add_keyboard(XkbConfig::default(), kb.repeat_delay, kb.repeat_rate)?
            }
        };
        if kb.numlock {
            let mut mods = keyboard.modifier_state();
            mods.num_lock = true;
            keyboard.set_modifier_state(mods);
        }
        seat.add_pointer();

        let socket_name = if create_socket {
            let socket = ListeningSocketSource::new_auto()?;
            let socket_name = socket.socket_name().to_string_lossy().into_owned();
            event_loop
                .insert_source(socket, |stream, _, state| {
                    if let Err(err) = state
                        .hecate
                        .display_handle
                        .insert_client(stream, Arc::new(ClientState::default()))
                    {
                        tracing::warn!("error adding wayland client: {err}");
                    }
                })
                .map_err(|e| anyhow::anyhow!("inserting wayland socket source: {e}"))?;
            Some(socket_name)
        } else {
            None
        };

        event_loop
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // SAFETY: the display is never dropped while the loop runs.
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|e| anyhow::anyhow!("inserting display source: {e}"))?;

        // Fallback frame callbacks for surfaces not on any output.
        event_loop
            .insert_source(
                smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_secs(1)),
                |_, _, state| {
                    state.hecate.send_fallback_frame_callbacks();
                    state.sync_fallback();
                    smithay::reexports::calloop::timer::TimeoutAction::ToDuration(Duration::from_secs(1))
                },
            )
            .map_err(|e| anyhow::anyhow!("inserting timer: {e}"))?;

        let mod_key = backend.mod_key();
        let binds = crate::config::load::resolved_binds(&config);
        let mouse_binds = crate::config::load::resolved_mouse_binds(&config);
        let cursor_manager = CursorManager::new(&config.cursor.theme, config.cursor.size);

        Ok(Hecate {
            wm: Wm::new(),
            file_config: config.clone(),
            overrides: Vec::new(),
            config_reload_failures: 0,
            last_config_error: None,
            ipc: None,
            identify_until: None,
            config,
            config_path,
            binds,
            mouse_binds,
            mod_key,
            keybinds_enabled: true,
            event_loop,
            stop_signal,
            display_handle: dh,
            socket_name,
            start_time: Instant::now(),
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            kde_decoration_state,
            layer_shell_state,
            session_lock_state,
            shm_state,
            output_manager_state,
            dmabuf_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            wlr_data_control_state,
            ext_data_control_state,
            viewporter_state,
            fractional_scale_state,
            presentation_state,
            xdg_activation_state,
            xdg_foreign_state,
            foreign_toplevel_state,
            ext_workspace_state,
            gamma_control_state,
            output_management_state,
            output_power_state,
            screencopy_state,
            focus_grab_state,
            tearing_control_state,
            content_type_state,
            fifo_state,
            commit_timing_state,
            commit_timer_armed: false,
            image_capture_source_state,
            output_capture_source_state,
            image_copy_capture_state,
            image_copy_sessions: Vec::new(),
            cursor_shape_state,
            single_pixel_buffer_state,
            idle_notifier_state,
            idle_inhibit_state,
            keyboard_shortcuts_inhibit_state,
            pointer_constraints_state,
            relative_pointer_state,
            pointer_gestures_state,
            security_context_state,
            text_input_state,
            input_method_state,
            virtual_keyboard_state,
            tablet_state,
            seat,
            popups: PopupManager::default(),
            output_state: HashMap::new(),
            output_heads_dirty: true,
            satellite: None,
            wake_outputs_on_input: false,
            pointer_location: (0., 0.).into(),
            cursor_status: CursorImageStatus::default_named(),
            cursor_manager,
            dnd_icon: None,
            idle_inhibitors: Vec::new(),
            shortcut_inhibitors: Vec::new(),
            suppressed_keys: Vec::new(),
            suppressed_buttons: Vec::new(),
            pending_mod_tap: None,
            reload_pending: false,
            config_watcher: None,
        })
    }

    pub fn queue_redraw(&mut self, output: &Output) {
        if let Some(state) = self.output_state.get_mut(output) {
            state.redraw_queued = true;
        }
    }

    pub fn queue_redraw_all(&mut self) {
        for state in self.output_state.values_mut() {
            state.redraw_queued = true;
        }
    }

    pub fn queued_outputs(&self) -> Vec<Output> {
        self.output_state
            .iter()
            .filter(|(_, s)| s.redraw_queued && !s.waiting_for_vblank)
            .map(|(o, _)| o.clone())
            .collect()
    }

    pub fn redraw_queued_outputs(&mut self, backend: &mut Backend) {
        for output in self.queued_outputs() {
            self.redraw(backend, &output);
        }
    }

    pub fn redraw(&mut self, backend: &mut Backend, output: &Output) {
        if let Some(state) = self.output_state.get_mut(output) {
            state.redraw_queued = false;
        }
        self.wm.advance_animations(output);
        let started = crate::utils::monotonic_time();
        let submitted = backend.render(self, output);
        let now = crate::utils::monotonic_time();
        if submitted && let Some(st) = self.output_state.get_mut(output) {
            st.frame_stats.submitted(now, now.saturating_sub(started));
            if !backend.waits_for_vblank() {
                // No vblank to wait for: the frame is done now.
                let refresh = output.current_mode().map(|m| m.refresh).unwrap_or(60_000).max(1);
                st.frame_stats
                    .presented(now, Duration::from_secs_f64(1000.0 / refresh as f64));
            }
        }
        if self.screencopy_state.has_pending_for(output) {
            backend.with_renderer(|r| self.render_for_screencopy_with_damage(r, output));
        }
        if let Some(state) = self.output_state.get_mut(output) {
            if submitted {
                state.waiting_for_vblank = backend.waits_for_vblank();
            }
            state.frame_callback_sequence = state.frame_callback_sequence.wrapping_add(1);
            if state.animations_running {
                state.redraw_queued = true;
            }
        }
        self.send_frame_callbacks(output);
    }

    /// Called by the backend once a submitted frame was presented.
    pub fn on_vblank(&mut self, output: &Output, presented: Duration, refresh: Duration) {
        if let Some(state) = self.output_state.get_mut(output) {
            state.waiting_for_vblank = false;
            state.frame_stats.presented(presented, refresh);
        }
    }

    pub fn send_frame_callbacks(&self, output: &Output) {
        let time = self.start_time.elapsed();
        let throttle = Some(Duration::from_millis(995));
        for window in self.wm.windows_on_output(output) {
            window.send_frame(output, time, throttle, |_, _| Some(output.clone()));
        }
        let map = smithay::desktop::layer_map_for_output(output);
        for layer in map.layers() {
            layer.send_frame(output, time, throttle, |_, _| Some(output.clone()));
        }
        drop(map);
        if let Some(lock) = self.wm.lock_surface(output) {
            smithay::desktop::utils::send_frames_surface_tree(
                lock.wl_surface(),
                output,
                time,
                throttle,
                |_, _| Some(output.clone()),
            );
        }
        if let CursorImageStatus::Surface(surface) = &self.cursor_status {
            smithay::desktop::utils::send_frames_surface_tree(surface, output, time, throttle, |_, _| {
                Some(output.clone())
            });
        }
        if let Some(surface) = &self.dnd_icon {
            smithay::desktop::utils::send_frames_surface_tree(surface, output, time, throttle, |_, _| {
                Some(output.clone())
            });
        }
    }

    fn send_fallback_frame_callbacks(&self) {
        // Windows on hidden workspaces still need occasional frame callbacks.
        let time = self.start_time.elapsed();
        let Some(output) = self.wm.outputs().next().cloned() else {
            return;
        };
        for window in self.wm.all_windows() {
            window.send_frame(&output, time, Some(Duration::from_secs(1)), |_, _| None);
        }
    }

    /// Popup surfaces of the given roots.
    pub fn popups_for(&self, roots: &[WlSurface]) -> Vec<WlSurface> {
        roots
            .iter()
            .flat_map(|r| PopupManager::popups_for_surface(r).map(|(p, _)| p.wl_surface().clone()))
            .collect()
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.wm.window_for_surface(surface)
    }

    /// Run a command with the compositor's environment.
    pub fn spawn(&self, argv: &[String]) {
        crate::utils::spawn(argv, self.socket_name.as_deref(), &self.config.environment);
    }
}

/// Interface names of the globals hecate advertises (for `hecate msg protocols`).
pub fn advertised_globals() -> Vec<String> {
    [
        "wl_compositor",
        "wl_subcompositor",
        "wl_shm",
        "wl_seat",
        "wl_output",
        "wl_data_device_manager",
        "xdg_wm_base",
        "zxdg_output_manager_v1",
        "zxdg_decoration_manager_v1",
        "org_kde_kwin_server_decoration_manager",
        "zwlr_layer_shell_v1",
        "ext_session_lock_manager_v1",
        "zwp_linux_dmabuf_v1",
        "wp_viewporter",
        "wp_fractional_scale_manager_v1",
        "wp_presentation",
        "xdg_activation_v1",
        "zxdg_exporter_v2",
        "zxdg_importer_v2",
        "wp_cursor_shape_manager_v1",
        "wp_single_pixel_buffer_manager_v1",
        "ext_idle_notifier_v1",
        "zwp_idle_inhibit_manager_v1",
        "zwp_keyboard_shortcuts_inhibit_manager_v1",
        "zwp_pointer_constraints_v1",
        "zwp_relative_pointer_manager_v1",
        "zwp_pointer_gestures_v1",
        "wp_security_context_manager_v1",
        "zwp_text_input_manager_v3",
        "zwp_input_method_manager_v2",
        "zwp_virtual_keyboard_manager_v1",
        "zwp_tablet_manager_v2",
        "zwp_primary_selection_device_manager_v1",
        "zwlr_data_control_manager_v1",
        "ext_data_control_manager_v1",
        "ext_foreign_toplevel_list_v1",
        "zwlr_foreign_toplevel_manager_v1",
        "ext_workspace_manager_v1",
        "zwlr_gamma_control_manager_v1",
        "zwlr_output_manager_v1",
        "zwlr_output_power_manager_v1",
        "zwlr_screencopy_manager_v1",
        "ext_image_copy_capture_manager_v1",
        "ext_output_image_capture_source_manager_v1",
        "ext_foreign_toplevel_image_capture_source_manager_v1",
        "hyprland_focus_grab_manager_v1",
        "wp_tearing_control_manager_v1",
        "wp_content_type_manager_v1",
        "wp_fifo_manager_v1",
        "wp_commit_timing_manager_v1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}
