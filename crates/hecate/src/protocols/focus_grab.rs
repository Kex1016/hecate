//! hyprland-focus-grab-v1: QuickShell's popups that close on an outside click.
//!
//! An active grab whitelists surfaces. A button press outside them clears the grab (the click
//! still goes through), and while it lasts the keyboard focus stays inside the whitelist.

use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;

#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    unused_imports,
    missing_docs,
    clippy::all
)]
pub mod server {
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-focus-grab-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("protocols/hyprland-focus-grab-v1.xml");
}

#[cfg(test)]
#[allow(
    non_upper_case_globals,
    non_camel_case_types,
    unused_imports,
    missing_docs,
    clippy::all
)]
pub mod client {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-focus-grab-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-focus-grab-v1.xml");
}

use server::hyprland_focus_grab_manager_v1::{self, HyprlandFocusGrabManagerV1};
use server::hyprland_focus_grab_v1::{self, HyprlandFocusGrabV1};

const VERSION: u32 = 1;

pub struct FocusGrabState {
    grabs: Vec<Grab>,
    /// The grab currently holding focus, if any.
    active: Option<HyprlandFocusGrabV1>,
}

struct Grab {
    resource: HyprlandFocusGrabV1,
    pending: Vec<WlSurface>,
    committed: Vec<WlSurface>,
}

pub struct FocusGrabGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl FocusGrabState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        display.create_global::<State, HyprlandFocusGrabManagerV1, _>(
            VERSION,
            FocusGrabGlobalData {
                filter: Box::new(filter),
            },
        );
        Self {
            grabs: Vec::new(),
            active: None,
        }
    }

    /// Whitelist of the active grab.
    pub fn active_surfaces(&self) -> Option<&[WlSurface]> {
        let active = self.active.as_ref()?;
        self.grabs
            .iter()
            .find(|g| g.resource == *active)
            .map(|g| g.committed.as_slice())
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    fn grab_mut(&mut self, resource: &HyprlandFocusGrabV1) -> Option<&mut Grab> {
        self.grabs.iter_mut().find(|g| g.resource == *resource)
    }
}

impl State {
    /// End the active grab, telling its client.
    pub fn clear_focus_grab(&mut self) {
        let fg = &mut self.hecate.focus_grab_state;
        let Some(active) = fg.active.take() else {
            return;
        };
        if let Some(g) = fg.grab_mut(&active) {
            g.committed.clear();
        }
        active.cleared();
        self.focus_topmost();
    }

    /// A button was pressed over `surface` (a root surface, or None over nothing).
    pub fn focus_grab_on_press(&mut self, surface: Option<&WlSurface>) {
        let inside = match (self.hecate.focus_grab_state.active_surfaces(), surface) {
            (None, _) => return,
            (Some(list), Some(s)) => list.contains(s),
            (Some(_), None) => false,
        };
        if !inside {
            self.clear_focus_grab();
        }
    }

    /// Drop dead surfaces from whitelists; an active grab left empty is cleared.
    pub fn refresh_focus_grab(&mut self) {
        let fg = &mut self.hecate.focus_grab_state;
        for g in &mut fg.grabs {
            g.pending.retain(|s| s.is_alive());
            g.committed.retain(|s| s.is_alive());
        }
        if fg.active_surfaces().is_some_and(|l| l.is_empty()) {
            self.clear_focus_grab();
        }
    }

    /// Keep the keyboard inside the whitelist of the active grab.
    fn focus_grab_keyboard(&mut self) {
        let Some(list) = self.hecate.focus_grab_state.active_surfaces() else {
            return;
        };
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        let current = keyboard.current_focus();
        if current.as_ref().is_some_and(|c| list.contains(c)) {
            return;
        }
        if let Some(target) = list.first().cloned() {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            keyboard.set_focus(self, Some(target), serial);
        }
    }

    fn commit_focus_grab(&mut self, resource: &HyprlandFocusGrabV1) {
        let fg = &mut self.hecate.focus_grab_state;
        let Some(g) = fg.grab_mut(resource) else {
            return;
        };
        g.committed = g.pending.clone();
        let empty = g.committed.is_empty();
        let is_active = fg.active.as_ref() == Some(resource);
        match (empty, is_active) {
            (true, true) => self.clear_focus_grab(),
            (true, false) => (),
            (false, true) => self.focus_grab_keyboard(),
            (false, false) => {
                // A new grab replaces the current one.
                self.clear_focus_grab();
                self.hecate.focus_grab_state.active = Some(resource.clone());
                self.focus_grab_keyboard();
            }
        }
    }
}

impl GlobalDispatch2<HyprlandFocusGrabManagerV1, State> for FocusGrabGlobalData {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<HyprlandFocusGrabManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, EmptyData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<HyprlandFocusGrabManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &HyprlandFocusGrabManagerV1,
        request: hyprland_focus_grab_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            hyprland_focus_grab_manager_v1::Request::CreateGrab { grab } => {
                let resource = data_init.init(grab, EmptyData);
                state.hecate.focus_grab_state.grabs.push(Grab {
                    resource,
                    pending: Vec::new(),
                    committed: Vec::new(),
                });
            }
            hyprland_focus_grab_manager_v1::Request::Destroy => (),
        }
    }
}

impl Dispatch2<HyprlandFocusGrabV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &HyprlandFocusGrabV1,
        request: hyprland_focus_grab_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            hyprland_focus_grab_v1::Request::AddSurface { surface } => {
                if let Some(g) = state.hecate.focus_grab_state.grab_mut(resource)
                    && !g.pending.contains(&surface)
                {
                    g.pending.push(surface);
                }
            }
            hyprland_focus_grab_v1::Request::RemoveSurface { surface } => {
                if let Some(g) = state.hecate.focus_grab_state.grab_mut(resource) {
                    g.pending.retain(|s| *s != surface);
                }
            }
            hyprland_focus_grab_v1::Request::Commit => state.commit_focus_grab(resource),
            hyprland_focus_grab_v1::Request::Destroy => (),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &HyprlandFocusGrabV1) {
        let fg = &mut state.hecate.focus_grab_state;
        fg.grabs.retain(|g| g.resource != *resource);
        if fg.active.as_ref() == Some(resource) {
            // Destroying an active grab removes it; nobody is left to tell.
            fg.active = None;
            state.focus_topmost();
        }
    }
}
