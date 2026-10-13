//! hyprland-toplevel-export-v1: window contents for QuickShell's `ScreencopyView` (window
//! previews in the overview, the Alt+Tab switcher and the screen sharing picker). QuickShell
//! captures toplevels through this protocol only.
//!
//! Frames are shm only. A `copy` waits for the window to change since this client's last frame
//! of it, unless it asks to ignore damage, so live previews follow the window's own frame rate.

use std::collections::HashMap;

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource};
use smithay::utils::{Physical, Point, Scale, Size, Transform};
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
    use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::*;
    use smithay::reexports::wayland_server;
    use smithay::reexports::wayland_server::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::__interfaces::*;
        use smithay::reexports::wayland_server::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_server_code!("protocols/hyprland-toplevel-export-v1.xml");
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
    use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::*;
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::__interfaces::*;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-export-v1.xml");
}

use server::hyprland_toplevel_export_frame_v1::{self, HyprlandToplevelExportFrameV1};
use server::hyprland_toplevel_export_manager_v1::{self, HyprlandToplevelExportManagerV1};

const VERSION: u32 = 2;
const FORMAT: wl_shm::Format = wl_shm::Format::Xrgb8888;

pub struct ToplevelExportState {
    frames: Vec<ExportFrame>,
    /// Per client and window, so a client's next frame only waits for new damage.
    damage: HashMap<(ClientId, u64), OutputDamageTracker>,
}

struct ExportFrame {
    resource: HyprlandToplevelExportFrameV1,
    window: u64,
    /// Size advertised in the `buffer` event.
    size: Size<i32, Physical>,
    /// Set by `copy`.
    copy: Option<(WlBuffer, bool)>,
}

pub struct ToplevelExportGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl ToplevelExportState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        display.create_global::<State, HyprlandToplevelExportManagerV1, _>(
            VERSION,
            ToplevelExportGlobalData {
                filter: Box::new(filter),
            },
        );
        Self {
            frames: Vec::new(),
            damage: HashMap::new(),
        }
    }
}

/// A mapped window, and the scale of the output it's on.
fn window_and_scale(state: &State, id: u64) -> Option<(Window, Scale<f64>)> {
    let w = state.hecate.wm.window_by_id(id)?;
    state.hecate.wm.workspace_of(&w)?;
    let scale = state
        .hecate
        .wm
        .output_for_window(&w)
        .map(|o| o.current_scale().fractional_scale())
        .unwrap_or(1.0);
    Some((w, scale.into()))
}

impl State {
    /// A new frame: tell the client what buffer to provide, or fail it.
    fn start_toplevel_export(&mut self, resource: HyprlandToplevelExportFrameV1, window: Option<u64>) {
        let Some((id, (w, scale))) = window.and_then(|id| Some((id, window_and_scale(self, id)?))) else {
            resource.failed();
            return;
        };
        let size = w.geometry().size.to_physical_precise_round(scale);
        if size.is_empty() {
            resource.failed();
            return;
        }
        resource.buffer(FORMAT, size.w as u32, size.h as u32, size.w as u32 * 4);
        resource.buffer_done();
        self.hecate.toplevel_export_state.frames.push(ExportFrame {
            resource,
            window: id,
            size,
            copy: None,
        });
    }

    /// Complete copies whose window has something new (or that ignore damage). Runs once per
    /// loop iteration.
    pub fn refresh_toplevel_export(&mut self) {
        let wm = &self.hecate.wm;
        let export = &mut self.hecate.toplevel_export_state;
        export.frames.retain(|f| f.resource.is_alive());
        export.damage.retain(|(_, id), _| wm.window_by_id(*id).is_some());
        if export.frames.iter().all(|f| f.copy.is_none()) {
            return;
        }
        let mut frames = std::mem::take(&mut self.hecate.toplevel_export_state.frames);
        frames.retain(|frame| {
            let Some((buffer, ignore_damage)) = frame.copy.clone() else {
                return true;
            };
            let Some((w, scale)) = window_and_scale(self, frame.window) else {
                frame.resource.failed();
                return false;
            };
            let size = w.geometry().size.to_physical_precise_round(scale);
            if size != frame.size {
                // Resized since the buffer was negotiated: the client asks again.
                frame.resource.failed();
                return false;
            }
            let Some(client) = frame.resource.client().map(|c| c.id()) else {
                return false;
            };
            let State { backend, hecate } = &mut *self;
            let done = backend.with_renderer(|renderer: &mut GlesRenderer| {
                let tracker = hecate
                    .toplevel_export_state
                    .damage
                    .entry((client, frame.window))
                    .or_insert_with(|| OutputDamageTracker::new(size, scale, Transform::Normal));
                if tracker.mode().clone().try_into().ok() != Some((size, scale, Transform::Normal)) {
                    *tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
                }
                let loc: Point<i32, Physical> = w.geometry().loc.upscale(-1).to_physical_precise_round(scale);
                let elements = AsRenderElements::<GlesRenderer>::render_elements::<
                    smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement<GlesRenderer>,
                >(&w, renderer, loc, scale, 1.0);
                let (damage, states) = tracker.damage_output(1, &elements).unwrap();
                if damage.is_none() && !ignore_damage {
                    return false;
                }
                let res = crate::render::capture::render_to_shm(
                    renderer, tracker, &buffer, FORMAT, &elements, states,
                );
                match res {
                    Ok(()) => {
                        let time = crate::utils::monotonic_time();
                        frame
                            .resource
                            .flags(hyprland_toplevel_export_frame_v1::Flags::empty());
                        frame.resource.damage(0, 0, size.w as u32, size.h as u32);
                        frame.resource.ready(
                            (time.as_secs() >> 32) as u32,
                            time.as_secs() as u32,
                            time.subsec_nanos(),
                        );
                    }
                    Err(err) => {
                        tracing::debug!("toplevel export failed: {err:?}");
                        frame.resource.failed();
                        *tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
                    }
                }
                true
            });
            // No renderer (yet): keep waiting.
            !done.unwrap_or(false)
        });
        frames.append(&mut self.hecate.toplevel_export_state.frames);
        self.hecate.toplevel_export_state.frames = frames;
    }
}

impl GlobalDispatch2<HyprlandToplevelExportManagerV1, State> for ToplevelExportGlobalData {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<HyprlandToplevelExportManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, EmptyData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<HyprlandToplevelExportManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &HyprlandToplevelExportManagerV1,
        request: hyprland_toplevel_export_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            // Hyprland's window address; ours is the window id (`hecate msg windows`).
            hyprland_toplevel_export_manager_v1::Request::CaptureToplevel { frame, handle, .. } => {
                let frame = data_init.init(frame, EmptyData);
                state.start_toplevel_export(frame, Some(handle as u64));
            }
            hyprland_toplevel_export_manager_v1::Request::CaptureToplevelWithWlrToplevelHandle {
                frame,
                handle,
                ..
            } => {
                let frame = data_init.init(frame, EmptyData);
                let id = state
                    .hecate
                    .foreign_toplevel_state
                    .window_id_for_wlr_handle(&handle);
                state.start_toplevel_export(frame, id);
            }
            hyprland_toplevel_export_manager_v1::Request::Destroy => (),
        }
    }
}

impl Dispatch2<HyprlandToplevelExportFrameV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &HyprlandToplevelExportFrameV1,
        request: hyprland_toplevel_export_frame_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            hyprland_toplevel_export_frame_v1::Request::Copy {
                buffer,
                ignore_damage,
            } => {
                let frames = &mut state.hecate.toplevel_export_state.frames;
                match frames.iter_mut().find(|f| f.resource == *resource) {
                    Some(f) if f.copy.is_none() => f.copy = Some((buffer, ignore_damage != 0)),
                    // Copied twice, or after failing.
                    _ => resource.failed(),
                }
            }
            hyprland_toplevel_export_frame_v1::Request::Destroy => (),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &HyprlandToplevelExportFrameV1) {
        state
            .hecate
            .toplevel_export_state
            .frames
            .retain(|f| f.resource != *resource);
    }
}
