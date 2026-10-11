//! wp-tearing-control-v1: a surface's hint that it prefers async presentation.

use std::sync::Mutex;

use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_manager_v1::{
    self, WpTearingControlManagerV1,
};
use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_v1::{
    self, PresentationHint, WpTearingControlV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource, WEnum, Weak};
use smithay::wayland::compositor::{Cacheable, with_states};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;

/// Double-buffered presentation hint of a surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TearingCachedState {
    pub wants_async: bool,
}

impl Cacheable for TearingCachedState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        *self
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

/// Whether a surface currently has a tearing control object.
struct HasTearingControl(Mutex<bool>);

pub struct TearingControlData(Mutex<Weak<WlSurface>>);

pub struct TearingControlState;

impl TearingControlState {
    pub fn new(display: &DisplayHandle) -> Self {
        display.create_global::<State, WpTearingControlManagerV1, _>(1, EmptyData);
        Self
    }
}

/// Whether the surface's committed hint asks for async (tearing) presentation.
pub fn wants_async(surface: &WlSurface) -> bool {
    with_states(surface, |states| {
        states
            .cached_state
            .get::<TearingCachedState>()
            .current()
            .wants_async
    })
}

impl GlobalDispatch2<WpTearingControlManagerV1, State> for EmptyData {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, EmptyData);
    }
}

impl Dispatch2<WpTearingControlManagerV1, State> for EmptyData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        resource: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } => {
                let taken = with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(|| HasTearingControl(Mutex::new(false)));
                    let flag = states.data_map.get::<HasTearingControl>().unwrap();
                    std::mem::replace(&mut *flag.0.lock().unwrap(), true)
                });
                if taken {
                    resource.post_error(
                        wp_tearing_control_manager_v1::Error::TearingControlExists,
                        "surface already has a tearing control",
                    );
                    return;
                }
                data_init.init(id, TearingControlData(Mutex::new(surface.downgrade())));
            }
            wp_tearing_control_manager_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch2<WpTearingControlV1, State> for TearingControlData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        let Ok(surface) = self.0.lock().unwrap().upgrade() else {
            return;
        };
        match request {
            wp_tearing_control_v1::Request::SetPresentationHint { hint } => {
                let wants_async = matches!(hint, WEnum::Value(PresentationHint::Async));
                with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<TearingCachedState>()
                        .pending()
                        .wants_async = wants_async;
                });
            }
            wp_tearing_control_v1::Request::Destroy => {
                // Back to vsync on the next commit, and the surface may get a new control.
                with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<TearingCachedState>()
                        .pending()
                        .wants_async = false;
                    if let Some(flag) = states.data_map.get::<HasTearingControl>() {
                        *flag.0.lock().unwrap() = false;
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, _state: &mut State, _client: ClientId, _resource: &WpTearingControlV1) {}
}
