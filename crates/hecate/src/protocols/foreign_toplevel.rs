//! wlr-foreign-toplevel-management and ext-foreign-toplevel-list.
//!
//! Adapted from niri (src/protocols/foreign_toplevel.rs), GPL-3.0-or-later.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use smithay::desktop::Window;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::foreign_toplevel_list::v1::server::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use smithay::reexports::wayland_protocols::ext::foreign_toplevel_list::v1::server::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;
use crate::wm::{Maximized, WindowExt};

const EXT_LIST_VERSION: u32 = 1;
const WLR_MANAGEMENT_VERSION: u32 = 3;

pub struct ForeignToplevelManagerState {
    display: DisplayHandle,
    ext_list_instances: HashSet<ExtForeignToplevelListV1>,
    wlr_management_instances: HashSet<ZwlrForeignToplevelManagerV1>,
    toplevels: HashMap<WlSurface, ToplevelData>,
}

struct ToplevelData {
    identifier: u64,
    title: String,
    app_id: String,
    states: Vec<u32>,
    output: Option<Output>,

    ext_list_instances: HashSet<ExtForeignToplevelHandleV1>,
    wlr_management_instances: HashMap<ZwlrForeignToplevelHandleV1, Vec<WlOutput>>,
}

#[derive(Clone)]
pub struct ForeignToplevelGlobalData {
    filter: Arc<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl ForeignToplevelManagerState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let global_data = ForeignToplevelGlobalData {
            filter: Arc::new(filter),
        };
        display.create_global::<State, ExtForeignToplevelListV1, _>(EXT_LIST_VERSION, global_data.clone());
        display.create_global::<State, ZwlrForeignToplevelManagerV1, _>(WLR_MANAGEMENT_VERSION, global_data);
        Self {
            display: display.clone(),
            ext_list_instances: HashSet::new(),
            wlr_management_instances: HashSet::new(),
            toplevels: HashMap::new(),
        }
    }
}

/// Identifier for ext-foreign-toplevel-list; window ids are never reused.
fn protocol_identifier(id: u64) -> String {
    format!("hecate-{id:016x}")
}

struct Snapshot {
    surface: WlSurface,
    identifier: u64,
    title: String,
    app_id: String,
    states: Vec<u32>,
    output: Option<Output>,
}

fn snapshot(state: &State, window: &Window, focused: bool) -> Snapshot {
    let d = window.data();
    let mut states = Vec::new();
    if d.maximized == Maximized::Both {
        states.push(zwlr_foreign_toplevel_handle_v1::State::Maximized as u32);
    }
    if d.minimized {
        states.push(zwlr_foreign_toplevel_handle_v1::State::Minimized as u32);
    }
    // Clients treat "activated" as keyboard focus (see niri), so only the focused window gets it.
    if focused {
        states.push(zwlr_foreign_toplevel_handle_v1::State::Activated as u32);
    }
    if d.fullscreen {
        states.push(zwlr_foreign_toplevel_handle_v1::State::Fullscreen as u32);
    }
    let identifier = d.id;
    drop(d);
    Snapshot {
        surface: window.toplevel_surface().wl_surface().clone(),
        identifier,
        title: window.title(),
        app_id: window.app_id(),
        states,
        output: state.hecate.wm.output_for_window(window).cloned(),
    }
}

/// Sync protocol state with the window list. Called once per loop iteration.
pub fn refresh(state: &mut State) {
    let focused = state.focused_window();
    let windows: Vec<Window> = state
        .hecate
        .wm
        .workspaces
        .iter()
        .flat_map(|ws| ws.windows.iter().chain(ws.minimized.iter()))
        .filter(|w| !w.data().rules.skip_taskbar)
        .cloned()
        .collect();

    // The focused window goes last so the previous one is deactivated first.
    let mut snaps: Vec<Snapshot> = windows
        .iter()
        .filter(|w| Some(*w) != focused.as_ref())
        .map(|w| snapshot(state, w, false))
        .collect();
    if let Some(f) = focused.as_ref().filter(|f| windows.contains(f)) {
        snaps.push(snapshot(state, f, true));
    }

    let protocol_state = &mut state.hecate.foreign_toplevel_state;

    protocol_state.toplevels.retain(|surface, data| {
        if snaps.iter().any(|s| &s.surface == surface) {
            return true;
        }
        for instance in &data.ext_list_instances {
            instance.closed();
        }
        for instance in data.wlr_management_instances.keys() {
            instance.closed();
        }
        false
    });

    for snap in snaps {
        refresh_toplevel(protocol_state, snap);
    }
}

pub fn on_output_bound(state: &mut State, output: &Output, wl_output: &WlOutput) {
    let Some(client) = wl_output.client() else {
        return;
    };
    for data in state.hecate.foreign_toplevel_state.toplevels.values_mut() {
        if data.output.as_ref() != Some(output) {
            continue;
        }
        for (instance, outputs) in &mut data.wlr_management_instances {
            if instance.client().as_ref() != Some(&client) {
                continue;
            }
            instance.output_enter(wl_output);
            instance.done();
            outputs.push(wl_output.clone());
        }
    }
}

fn refresh_toplevel(protocol_state: &mut ForeignToplevelManagerState, snap: Snapshot) {
    match protocol_state.toplevels.entry(snap.surface) {
        Entry::Occupied(entry) => {
            let data = entry.into_mut();

            let new_title = (data.title != snap.title).then(|| snap.title.clone());
            let new_app_id = (data.app_id != snap.app_id).then(|| snap.app_id.clone());
            let states_changed = data.states != snap.states;
            let output_changed = data.output != snap.output;
            data.title = snap.title;
            data.app_id = snap.app_id;
            data.states = snap.states;
            data.output = snap.output;

            if new_title.is_some() || new_app_id.is_some() {
                for instance in &data.ext_list_instances {
                    if let Some(t) = &new_title {
                        instance.title(t.clone());
                    }
                    if let Some(a) = &new_app_id {
                        instance.app_id(a.clone());
                    }
                    instance.done();
                }
            }

            if new_title.is_some() || new_app_id.is_some() || states_changed || output_changed {
                for (instance, outputs) in &mut data.wlr_management_instances {
                    if let Some(t) = &new_title {
                        instance.title(t.clone());
                    }
                    if let Some(a) = &new_app_id {
                        instance.app_id(a.clone());
                    }
                    if states_changed {
                        instance.state(states_bytes(&data.states));
                    }
                    if output_changed {
                        for wl_output in outputs.drain(..) {
                            instance.output_leave(&wl_output);
                        }
                        if let (Some(output), Some(client)) = (&data.output, instance.client()) {
                            for wl_output in output.client_outputs(&client) {
                                instance.output_enter(&wl_output);
                                outputs.push(wl_output);
                            }
                        }
                    }
                    instance.done();
                }
            }

            for outputs in data.wlr_management_instances.values_mut() {
                outputs.retain(|x| x.is_alive());
            }
        }
        Entry::Vacant(entry) => {
            let mut data = ToplevelData {
                identifier: snap.identifier,
                title: snap.title,
                app_id: snap.app_id,
                states: snap.states,
                output: snap.output,
                ext_list_instances: HashSet::new(),
                wlr_management_instances: HashMap::new(),
            };
            for manager in &protocol_state.ext_list_instances {
                if let Some(client) = manager.client() {
                    data.add_ext_instance(&protocol_state.display, &client, manager);
                }
            }
            for manager in &protocol_state.wlr_management_instances {
                if let Some(client) = manager.client() {
                    data.add_wlr_instance(&protocol_state.display, &client, manager);
                }
            }
            entry.insert(data);
        }
    }
}

fn states_bytes(states: &[u32]) -> Vec<u8> {
    states.iter().flat_map(|x| x.to_ne_bytes()).collect()
}

impl ToplevelData {
    fn add_ext_instance(
        &mut self,
        handle: &DisplayHandle,
        client: &Client,
        manager: &ExtForeignToplevelListV1,
    ) {
        let Ok(toplevel) = client.create_resource::<ExtForeignToplevelHandleV1, _, State>(
            handle,
            manager.version(),
            EmptyData,
        ) else {
            return;
        };
        manager.toplevel(&toplevel);
        toplevel.identifier(protocol_identifier(self.identifier));
        toplevel.title(self.title.clone());
        toplevel.app_id(self.app_id.clone());
        toplevel.done();
        self.ext_list_instances.insert(toplevel);
    }

    fn add_wlr_instance(
        &mut self,
        handle: &DisplayHandle,
        client: &Client,
        manager: &ZwlrForeignToplevelManagerV1,
    ) {
        let Ok(toplevel) = client.create_resource::<ZwlrForeignToplevelHandleV1, _, State>(
            handle,
            manager.version(),
            EmptyData,
        ) else {
            return;
        };
        manager.toplevel(&toplevel);
        toplevel.title(self.title.clone());
        toplevel.app_id(self.app_id.clone());
        toplevel.state(states_bytes(&self.states));
        let mut outputs = Vec::new();
        if let Some(output) = &self.output {
            for wl_output in output.client_outputs(client) {
                toplevel.output_enter(&wl_output);
                outputs.push(wl_output);
            }
        }
        toplevel.done();
        self.wlr_management_instances.insert(toplevel, outputs);
    }
}

impl GlobalDispatch2<ExtForeignToplevelListV1, State> for ForeignToplevelGlobalData {
    fn bind(
        &self,
        state: &mut State,
        handle: &DisplayHandle,
        client: &Client,
        resource: New<ExtForeignToplevelListV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(resource, EmptyData);
        let state = &mut state.hecate.foreign_toplevel_state;
        for data in state.toplevels.values_mut() {
            data.add_ext_instance(handle, client, &manager);
        }
        state.ext_list_instances.insert(manager);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ExtForeignToplevelListV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ExtForeignToplevelListV1,
        request: ext_foreign_toplevel_list_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_foreign_toplevel_list_v1::Request::Stop => {
                resource.finished();
                state
                    .hecate
                    .foreign_toplevel_state
                    .ext_list_instances
                    .remove(resource);
            }
            ext_foreign_toplevel_list_v1::Request::Destroy => {}
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtForeignToplevelListV1) {
        state
            .hecate
            .foreign_toplevel_state
            .ext_list_instances
            .remove(resource);
    }
}

impl Dispatch2<ExtForeignToplevelHandleV1, State> for EmptyData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &ExtForeignToplevelHandleV1,
        request: ext_foreign_toplevel_handle_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_foreign_toplevel_handle_v1::Request::Destroy => {}
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtForeignToplevelHandleV1) {
        for data in state.hecate.foreign_toplevel_state.toplevels.values_mut() {
            data.ext_list_instances.remove(resource);
        }
    }
}

impl GlobalDispatch2<ZwlrForeignToplevelManagerV1, State> for ForeignToplevelGlobalData {
    fn bind(
        &self,
        state: &mut State,
        handle: &DisplayHandle,
        client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(resource, EmptyData);
        let state = &mut state.hecate.foreign_toplevel_state;
        for data in state.toplevels.values_mut() {
            data.add_wlr_instance(handle, client, &manager);
        }
        state.wlr_management_instances.insert(manager);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ZwlrForeignToplevelManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ZwlrForeignToplevelManagerV1,
        request: zwlr_foreign_toplevel_manager_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_foreign_toplevel_manager_v1::Request::Stop => {
                resource.finished();
                state
                    .hecate
                    .foreign_toplevel_state
                    .wlr_management_instances
                    .remove(resource);
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ZwlrForeignToplevelManagerV1) {
        state
            .hecate
            .foreign_toplevel_state
            .wlr_management_instances
            .remove(resource);
    }
}

impl Dispatch2<ZwlrForeignToplevelHandleV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ZwlrForeignToplevelHandleV1,
        request: zwlr_foreign_toplevel_handle_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        let Some(surface) = state
            .hecate
            .foreign_toplevel_state
            .toplevels
            .iter()
            .find(|(_, data)| data.wlr_management_instances.contains_key(resource))
            .map(|(s, _)| s.clone())
        else {
            return;
        };
        let Some(window) = state.hecate.window_for_surface(&surface) else {
            return;
        };

        match request {
            zwlr_foreign_toplevel_handle_v1::Request::SetMaximized => {
                state.set_maximized(&window, Maximized::Both)
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMaximized => {
                state.set_maximized(&window, Maximized::No)
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetMinimized => state.set_minimized(&window, true),
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMinimized => state.set_minimized(&window, false),
            zwlr_foreign_toplevel_handle_v1::Request::Activate { .. } => state.activate_window(&window),
            zwlr_foreign_toplevel_handle_v1::Request::Close => window.toplevel_surface().send_close(),
            zwlr_foreign_toplevel_handle_v1::Request::SetRectangle { .. } => (),
            zwlr_foreign_toplevel_handle_v1::Request::Destroy => (),
            zwlr_foreign_toplevel_handle_v1::Request::SetFullscreen { output } => {
                let output = output.as_ref().and_then(Output::from_resource);
                state.set_fullscreen(&window, true, output);
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetFullscreen => {
                state.set_fullscreen(&window, false, None)
            }
            _ => unreachable!(),
        }
        state.hecate.queue_redraw_all();
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ZwlrForeignToplevelHandleV1) {
        for data in state.hecate.foreign_toplevel_state.toplevels.values_mut() {
            data.wlr_management_instances.remove(resource);
        }
    }
}
