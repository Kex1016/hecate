//! ext-background-effect-v1: clients ask for blur behind parts of their surfaces.

use std::sync::Mutex;

use smithay::reexports::wayland_protocols::ext::background_effect::v1::server::ext_background_effect_manager_v1::{
    self, Capability, ExtBackgroundEffectManagerV1,
};
use smithay::reexports::wayland_protocols::ext::background_effect::v1::server::ext_background_effect_surface_v1::{
    self, ExtBackgroundEffectSurfaceV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource, Weak};
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::compositor::{Cacheable, RectangleKind, get_region_attributes, with_states};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;

/// Double-buffered blur region of a surface, surface-local. None: no blur.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlurRegionCachedState {
    pub region: Option<Vec<Rectangle<i32, Logical>>>,
}

impl Cacheable for BlurRegionCachedState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

/// Whether a surface currently has a background effect object.
struct HasBackgroundEffect(Mutex<bool>);

pub struct BackgroundEffectData(Mutex<Weak<WlSurface>>);

pub struct BackgroundEffectState {
    managers: Vec<ExtBackgroundEffectManagerV1>,
    blur: bool,
}

impl BackgroundEffectState {
    pub fn new(display: &DisplayHandle, blur: bool) -> Self {
        display.create_global::<State, ExtBackgroundEffectManagerV1, _>(1, EmptyData);
        BackgroundEffectState {
            managers: Vec::new(),
            blur,
        }
    }

    fn capabilities(&self) -> Capability {
        if self.blur {
            Capability::Blur
        } else {
            Capability::empty()
        }
    }

    /// Blur was switched on or off in the config: tell every client.
    pub fn set_blur(&mut self, blur: bool) {
        self.managers.retain(|m| m.is_alive());
        if self.blur == blur {
            return;
        }
        self.blur = blur;
        for m in &self.managers {
            m.capabilities(self.capabilities());
        }
    }
}

/// The committed blur region of a surface, surface-local.
pub fn blur_region(surface: &WlSurface) -> Option<Vec<Rectangle<i32, Logical>>> {
    with_states(surface, |states| {
        states
            .cached_state
            .get::<BlurRegionCachedState>()
            .current()
            .region
            .clone()
    })
}

impl GlobalDispatch2<ExtBackgroundEffectManagerV1, State> for EmptyData {
    fn bind(
        &self,
        state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ExtBackgroundEffectManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(resource, EmptyData);
        let s = &mut state.hecate.background_effect_state;
        manager.capabilities(s.capabilities());
        s.managers.push(manager);
    }
}

impl Dispatch2<ExtBackgroundEffectManagerV1, State> for EmptyData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        resource: &ExtBackgroundEffectManagerV1,
        request: ext_background_effect_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_background_effect_manager_v1::Request::GetBackgroundEffect { id, surface } => {
                let taken = with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(|| HasBackgroundEffect(Mutex::new(false)));
                    let flag = states.data_map.get::<HasBackgroundEffect>().unwrap();
                    std::mem::replace(&mut *flag.0.lock().unwrap(), true)
                });
                if taken {
                    resource.post_error(
                        ext_background_effect_manager_v1::Error::BackgroundEffectExists,
                        "surface already has a background effect object",
                    );
                    return;
                }
                data_init.init(id, BackgroundEffectData(Mutex::new(surface.downgrade())));
            }
            ext_background_effect_manager_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtBackgroundEffectManagerV1) {
        state
            .hecate
            .background_effect_state
            .managers
            .retain(|m| m != resource);
    }
}

/// A wl_region's area as plain rectangles.
fn region_rects(
    region: &smithay::reexports::wayland_server::protocol::wl_region::WlRegion,
) -> Vec<Rectangle<i32, Logical>> {
    let mut rects: Vec<Rectangle<i32, Logical>> = Vec::new();
    for (kind, rect) in get_region_attributes(region).rects {
        match kind {
            RectangleKind::Add => rects.push(rect),
            RectangleKind::Subtract => rects = Rectangle::subtract_rects_many(rects, [rect]),
        }
    }
    rects
}

impl Dispatch2<ExtBackgroundEffectSurfaceV1, State> for BackgroundEffectData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        resource: &ExtBackgroundEffectSurfaceV1,
        request: ext_background_effect_surface_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        let surface = self.0.lock().unwrap().upgrade();
        match request {
            ext_background_effect_surface_v1::Request::SetBlurRegion { region } => {
                let Ok(surface) = surface else {
                    resource.post_error(
                        ext_background_effect_surface_v1::Error::SurfaceDestroyed,
                        "the surface was destroyed",
                    );
                    return;
                };
                // The region is copied now; later changes to the wl_region don't apply.
                let rects = region.as_ref().map(region_rects);
                with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<BlurRegionCachedState>()
                        .pending()
                        .region = rects;
                });
            }
            ext_background_effect_surface_v1::Request::Destroy => {
                let Ok(surface) = surface else {
                    return;
                };
                // No blur from the next commit on, and the surface may get a new object.
                with_states(&surface, |states| {
                    states
                        .cached_state
                        .get::<BlurRegionCachedState>()
                        .pending()
                        .region = None;
                    if let Some(flag) = states.data_map.get::<HasBackgroundEffect>() {
                        *flag.0.lock().unwrap() = false;
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, _state: &mut State, _client: ClientId, _resource: &ExtBackgroundEffectSurfaceV1) {}
}
