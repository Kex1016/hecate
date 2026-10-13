//! Gaming policy: which windows want tearing, when VRR turns on, and what `msg gaming` reports.

use hecate_ipc as ipc;
use smithay::desktop::Window;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::wp::content_type::v1::server::wp_content_type_v1;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;
use smithay::wayland::content_type::ContentTypeSurfaceCachedState;
use smithay::wayland::pointer_constraints::{PointerConstraint, with_pointer_constraint};

use crate::config::{Tearing, Vrr};
use crate::state::State;
use crate::wm::WindowExt;

/// wp-content-type of a surface, if set.
pub fn content_type(surface: &WlSurface) -> Option<&'static str> {
    let t = with_states(surface, |states| {
        *states
            .cached_state
            .get::<ContentTypeSurfaceCachedState>()
            .current()
            .content_type()
    });
    match t {
        wp_content_type_v1::Type::Photo => Some("photo"),
        wp_content_type_v1::Type::Video => Some("video"),
        wp_content_type_v1::Type::Game => Some("game"),
        _ => None,
    }
}

impl State {
    /// A window asks for tearing: window rule, tearing-control hint, or content type `game`.
    pub fn window_wants_tearing(&self, w: &Window) -> bool {
        let surface = w.toplevel_surface().wl_surface();
        w.data().rules.tearing
            || crate::protocols::tearing_control::wants_async(surface)
            || content_type(surface) == Some("game")
    }

    /// The fullscreen window on top of an output, if the topmost visible window is fullscreen.
    pub fn fullscreen_window_on(&self, output: &Output) -> Option<Window> {
        self.hecate
            .wm
            .windows_on_output(output)
            .last()
            .filter(|w| w.data().fullscreen)
            .cloned()
    }

    /// Whether the config lets the output tear right now.
    pub fn tearing_allowed(&self, output: &Output) -> bool {
        let fullscreen = self.fullscreen_window_on(output);
        let wants = |w: &Window| self.window_wants_tearing(w);
        match self.hecate.config.gaming.allow_tearing {
            Tearing::Never => false,
            Tearing::Fullscreen => fullscreen.as_ref().is_some_and(wants),
            Tearing::Always => self.hecate.wm.windows_on_output(output).last().is_some_and(wants),
        }
    }

    /// Whether VRR should be on for an output given its config and what's on screen.
    pub fn vrr_wanted(&self, output: &Output) -> bool {
        match self
            .hecate
            .config
            .outputs
            .get(&output.name())
            .and_then(|o| o.adaptive_sync)
            .unwrap_or_default()
        {
            Vrr::Off => false,
            Vrr::On => true,
            Vrr::Fullscreen => self.fullscreen_window_on(output).is_some(),
        }
    }

    /// Turn VRR on or off where `adaptive_sync = "fullscreen"` and fullscreen state changed.
    pub fn refresh_vrr(&mut self) {
        let outputs: Vec<Output> = self.hecate.wm.outputs().cloned().collect();
        for o in outputs {
            let want = self.vrr_wanted(&o);
            let Some(st) = self.hecate.output_state.get_mut(&o) else {
                continue;
            };
            if st.vrr_applied == Some(want) {
                continue;
            }
            st.vrr_applied = Some(want);
            self.backend.set_vrr(&o, want);
        }
    }

    pub fn ipc_gaming(&self) -> ipc::Gaming {
        let outputs = self
            .hecate
            .wm
            .outputs()
            .map(|o| {
                let fullscreen = self.fullscreen_window_on(o);
                let blocker = if !self.hecate.config.gaming.direct_scanout {
                    Some("gaming.direct_scanout = false".to_string())
                } else if fullscreen.is_none() {
                    Some("no fullscreen window on top".to_string())
                } else {
                    None
                };
                let vrr_supported = self.backend.vrr_supported(o);
                ipc::GamingOutput {
                    name: o.name(),
                    refresh_hz: o.current_mode().map(|m| m.refresh as f64 / 1000.0).unwrap_or(0.0),
                    vrr_supported,
                    vrr_enabled: vrr_supported && self.vrr_wanted(o),
                    tearing_allowed: self.tearing_allowed(o),
                    direct_scanout: blocker.is_none(),
                    direct_scanout_blocker: blocker,
                }
            })
            .collect();
        let pointer = self.hecate.seat.get_pointer();
        let windows = self
            .hecate
            .wm
            .visible_windows()
            .iter()
            .map(|w| {
                let surface = w.toplevel_surface().wl_surface();
                let (locked, confined) = pointer
                    .as_ref()
                    .map(|p| {
                        with_pointer_constraint(surface, p, |c| match c.as_deref() {
                            Some(c @ PointerConstraint::Locked(_)) if c.is_active() => (true, false),
                            Some(c @ PointerConstraint::Confined(_)) if c.is_active() => (false, true),
                            _ => (false, false),
                        })
                    })
                    .unwrap_or((false, false));
                ipc::GamingWindow {
                    id: w.wid(),
                    app_id: Some(w.app_id()).filter(|a| !a.is_empty()),
                    content_type: content_type(surface).map(String::from),
                    tearing_requested: self.window_wants_tearing(w),
                    pointer_locked: locked,
                    pointer_confined: confined,
                    fullscreen: w.data().fullscreen,
                }
            })
            .collect();
        ipc::Gaming { outputs, windows }
    }
}
