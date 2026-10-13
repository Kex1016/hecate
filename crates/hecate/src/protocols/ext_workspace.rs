//! ext-workspace-v1.
//!
//! Adapted from niri (src/protocols/ext_workspace.rs), GPL-3.0-or-later.
//!
//! Mapping to hecate:
//!
//! - `per_output` scope: one workspace group per output.
//! - `global` scope: a single group that has entered every output.
//! - Coordinates: one dimension, the workspace index within its group.
//! - Id: the configured name for named workspaces, unset otherwise.
//! - Name: the workspace label (name, or 1-based index).

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::mem;

use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_group_handle_v1::{
    self, ExtWorkspaceGroupHandleV1,
};
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_handle_v1::{
    self, ExtWorkspaceHandleV1,
};
use smithay::reexports::wayland_protocols::ext::workspace::v1::server::ext_workspace_manager_v1::{
    self, ExtWorkspaceManagerV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;
use crate::wm::WindowExt;

const VERSION: u32 = 1;

/// Workspace group key: `None` for the global group, the output name in `per_output` scope.
type GroupKey = Option<String>;

pub struct ExtWorkspaceManagerState {
    display: DisplayHandle,
    /// Manager instances and the activations queued until `commit`.
    instances: HashMap<ExtWorkspaceManagerV1, Vec<u64>>,
    groups: HashMap<GroupKey, GroupData>,
    workspaces: HashMap<u64, WorkspaceData>,
}

struct GroupData {
    outputs: Vec<Output>,
    instances: Vec<ExtWorkspaceGroupHandleV1>,
}

#[derive(PartialEq, Eq)]
pub struct WrappedManager(ExtWorkspaceManagerV1);

struct WorkspaceData {
    id: Option<String>,
    name: String,
    index: u32,
    state: ext_workspace_handle_v1::State,
    group: GroupKey,
    instances: Vec<ExtWorkspaceHandleV1>,
}

pub struct ExtWorkspaceGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl ExtWorkspaceManagerState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let global_data = ExtWorkspaceGlobalData {
            filter: Box::new(filter),
        };
        display.create_global::<State, ExtWorkspaceManagerV1, _>(VERSION, global_data);
        Self {
            display: display.clone(),
            instances: HashMap::new(),
            groups: HashMap::new(),
            workspaces: HashMap::new(),
        }
    }
}

struct WsSnapshot {
    id: u64,
    name: Option<String>,
    label: String,
    index: u32,
    state: ext_workspace_handle_v1::State,
    group: GroupKey,
}

/// Sync protocol state with the workspace list. Called once per loop iteration.
pub fn refresh(state: &mut State) {
    let wm = &state.hecate.wm;

    // Current groups and their outputs.
    let mut current_groups: Vec<(GroupKey, Vec<Output>)> = Vec::new();
    for output in wm.outputs() {
        let key = match wm.scope {
            crate::config::WorkspaceScope::Global => None,
            crate::config::WorkspaceScope::PerOutput => Some(output.name()),
        };
        match current_groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, outs)) => outs.push(output.clone()),
            None => current_groups.push((key, vec![output.clone()])),
        }
    }

    let snaps: Vec<WsSnapshot> = wm
        .workspaces
        .iter()
        .map(|ws| {
            let mut st = ext_workspace_handle_v1::State::empty();
            if wm.is_workspace_active(ws.id) {
                st |= ext_workspace_handle_v1::State::Active;
            }
            if ws
                .windows
                .iter()
                .chain(ws.minimized.iter())
                .any(|w| w.data().urgent)
            {
                st |= ext_workspace_handle_v1::State::Urgent;
            }
            WsSnapshot {
                id: ws.id,
                name: ws.name.clone(),
                label: wm.workspace_label(ws.id),
                index: wm.workspace_index(ws.id).unwrap_or(1).saturating_sub(1) as u32,
                state: st,
                group: ws.output.clone(),
            }
        })
        .collect();

    let ps = &mut state.hecate.ext_workspace_state;
    let mut changed = false;

    // Removed workspaces.
    ps.workspaces.retain(|id, data| {
        if snaps.iter().any(|s| s.id == *id) {
            return true;
        }
        send_enter_leave(&ps.groups, data, false);
        for instance in &data.instances {
            instance.removed();
        }
        changed = true;
        false
    });

    // Removed groups.
    ps.groups.retain(|key, group| {
        if current_groups.iter().any(|(k, _)| k == key) {
            return true;
        }
        for handle in &group.instances {
            let manager: &WrappedManager = handle.data().unwrap();
            for ws in ps.workspaces.values().filter(|ws| ws.group == *key) {
                for instance in ws.instances.iter().filter(|i| i.data() == Some(manager)) {
                    handle.workspace_leave(instance);
                }
            }
            handle.removed();
        }
        changed = true;
        false
    });

    // Output changes of existing groups (global scope gains/loses outputs).
    for (key, outputs) in &current_groups {
        if let Some(group) = ps.groups.get_mut(key) {
            changed |= group.set_outputs(outputs);
        }
    }

    for snap in snaps {
        changed |= refresh_workspace(ps, snap);
    }

    // New groups.
    for (key, outputs) in current_groups {
        if ps.groups.contains_key(&key) {
            continue;
        }
        let mut group = GroupData {
            outputs,
            instances: Vec::new(),
        };
        for manager in ps.instances.keys() {
            if let Some(client) = manager.client() {
                group.add_instance(&ps.display, &client, manager);
            }
        }
        for handle in &group.instances {
            let manager: &WrappedManager = handle.data().unwrap();
            for ws in ps.workspaces.values().filter(|ws| ws.group == key) {
                for instance in ws.instances.iter().filter(|i| i.data() == Some(manager)) {
                    handle.workspace_enter(instance);
                }
            }
        }
        ps.groups.insert(key, group);
        changed = true;
    }

    if changed {
        for manager in ps.instances.keys() {
            manager.done();
        }
    }
}

pub fn on_output_bound(state: &mut State, output: &Output, wl_output: &WlOutput) {
    let Some(client) = wl_output.client() else {
        return;
    };
    let ps = &mut state.hecate.ext_workspace_state;
    let mut sent = false;
    for group in ps.groups.values().filter(|g| g.outputs.contains(output)) {
        for handle in group
            .instances
            .iter()
            .filter(|h| h.client().as_ref() == Some(&client))
        {
            handle.output_enter(wl_output);
            sent = true;
        }
    }
    if sent {
        for manager in ps
            .instances
            .keys()
            .filter(|m| m.client().as_ref() == Some(&client))
        {
            manager.done();
        }
    }
}

fn send_enter_leave(groups: &HashMap<GroupKey, GroupData>, data: &WorkspaceData, enter: bool) {
    let Some(group) = groups.get(&data.group) else {
        return;
    };
    for handle in &group.instances {
        let manager: &WrappedManager = handle.data().unwrap();
        for instance in data.instances.iter().filter(|i| i.data() == Some(manager)) {
            if enter {
                handle.workspace_enter(instance);
            } else {
                handle.workspace_leave(instance);
            }
        }
    }
}

fn coordinates(index: u32) -> Vec<u8> {
    index.to_ne_bytes().to_vec()
}

fn refresh_workspace(ps: &mut ExtWorkspaceManagerState, snap: WsSnapshot) -> bool {
    match ps.workspaces.entry(snap.id) {
        Entry::Occupied(entry) => {
            let data = entry.into_mut();

            // The id is immutable once sent: a rename recreates the handles.
            let recreate = data.id.is_some() && data.id != snap.name;
            let id_set = data.id.is_none() && snap.name.is_some();
            data.id = snap.name;

            let name_changed = data.name != snap.label;
            data.name = snap.label;
            let index_changed = data.index != snap.index;
            data.index = snap.index;
            let state_changed = data.state != snap.state;
            data.state = snap.state;

            let group_changed = data.group != snap.group;
            if group_changed {
                send_enter_leave(&ps.groups, data, false);
                data.group = snap.group;
            }

            if recreate {
                send_enter_leave(&ps.groups, data, false);
                for instance in data.instances.drain(..) {
                    instance.removed();
                }
                for manager in ps.instances.keys() {
                    if let Some(client) = manager.client() {
                        data.add_instance(&ps.display, &client, manager);
                    }
                }
                send_enter_leave(&ps.groups, data, true);
                return true;
            }

            if group_changed {
                send_enter_leave(&ps.groups, data, true);
            }

            for instance in &data.instances {
                if id_set {
                    instance.id(data.id.clone().unwrap());
                }
                if name_changed {
                    instance.name(data.name.clone());
                }
                if index_changed {
                    instance.coordinates(coordinates(data.index));
                }
                if state_changed {
                    instance.state(data.state);
                }
            }

            group_changed || id_set || name_changed || index_changed || state_changed
        }
        Entry::Vacant(entry) => {
            let mut data = WorkspaceData {
                id: snap.name,
                name: snap.label,
                index: snap.index,
                state: snap.state,
                group: snap.group,
                instances: Vec::new(),
            };
            for manager in ps.instances.keys() {
                if let Some(client) = manager.client() {
                    data.add_instance(&ps.display, &client, manager);
                }
            }
            send_enter_leave(&ps.groups, &data, true);
            entry.insert(data);
            true
        }
    }
}

impl GroupData {
    fn add_instance(
        &mut self,
        handle: &DisplayHandle,
        client: &Client,
        manager: &ExtWorkspaceManagerV1,
    ) -> Option<&ExtWorkspaceGroupHandleV1> {
        let group = client
            .create_resource::<ExtWorkspaceGroupHandleV1, _, State>(
                handle,
                manager.version(),
                WrappedManager(manager.clone()),
            )
            .ok()?;
        manager.workspace_group(&group);
        group.capabilities(ext_workspace_group_handle_v1::GroupCapabilities::empty());
        for output in &self.outputs {
            for wl_output in output.client_outputs(client) {
                group.output_enter(&wl_output);
            }
        }
        self.instances.push(group);
        self.instances.last()
    }

    /// Update the group's outputs, sending output_enter/leave. Returns whether anything changed.
    fn set_outputs(&mut self, outputs: &[Output]) -> bool {
        let added: Vec<Output> = outputs
            .iter()
            .filter(|o| !self.outputs.contains(o))
            .cloned()
            .collect();
        let removed: Vec<Output> = self
            .outputs
            .iter()
            .filter(|o| !outputs.contains(o))
            .cloned()
            .collect();
        if added.is_empty() && removed.is_empty() {
            return false;
        }
        for handle in &self.instances {
            let Some(client) = handle.client() else {
                continue;
            };
            for o in &removed {
                for wl_output in o.client_outputs(&client) {
                    handle.output_leave(&wl_output);
                }
            }
            for o in &added {
                for wl_output in o.client_outputs(&client) {
                    handle.output_enter(&wl_output);
                }
            }
        }
        self.outputs = outputs.to_vec();
        true
    }
}

impl WorkspaceData {
    fn add_instance(
        &mut self,
        handle: &DisplayHandle,
        client: &Client,
        manager: &ExtWorkspaceManagerV1,
    ) -> Option<&ExtWorkspaceHandleV1> {
        let workspace = client
            .create_resource::<ExtWorkspaceHandleV1, _, State>(
                handle,
                manager.version(),
                WrappedManager(manager.clone()),
            )
            .ok()?;
        manager.workspace(&workspace);
        if let Some(id) = self.id.clone() {
            workspace.id(id);
        }
        workspace.name(self.name.clone());
        workspace.coordinates(coordinates(self.index));
        workspace.state(self.state);
        workspace.capabilities(ext_workspace_handle_v1::WorkspaceCapabilities::Activate);
        self.instances.push(workspace);
        self.instances.last()
    }
}

impl GlobalDispatch2<ExtWorkspaceManagerV1, State> for ExtWorkspaceGlobalData {
    fn bind(
        &self,
        state: &mut State,
        handle: &DisplayHandle,
        client: &Client,
        resource: New<ExtWorkspaceManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(resource, EmptyData);
        let ps = &mut state.hecate.ext_workspace_state;

        let mut new_workspaces: HashMap<GroupKey, Vec<ExtWorkspaceHandleV1>> = HashMap::new();
        for data in ps.workspaces.values_mut() {
            let group = data.group.clone();
            if let Some(ws) = data.add_instance(handle, client, &manager) {
                new_workspaces.entry(group).or_default().push(ws.clone());
            }
        }
        for (key, group) in &mut ps.groups {
            if let Some(g) = group.add_instance(handle, client, &manager) {
                for ws in new_workspaces.get(key).into_iter().flatten() {
                    g.workspace_enter(ws);
                }
            }
        }

        manager.done();
        ps.instances.insert(manager, Vec::new());
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ExtWorkspaceManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ExtWorkspaceManagerV1,
        request: ext_workspace_manager_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_workspace_manager_v1::Request::Commit => {
                let Some(actions) = state.hecate.ext_workspace_state.instances.get_mut(resource) else {
                    return;
                };
                for id in mem::take(actions) {
                    state.activate_workspace_by_id(id);
                }
            }
            ext_workspace_manager_v1::Request::Stop => {
                resource.finished();
                let ps = &mut state.hecate.ext_workspace_state;
                ps.instances.retain(|x, _| x != resource);
                let manager = WrappedManager(resource.clone());
                for data in ps.groups.values_mut() {
                    data.instances.retain(|i| i.data() != Some(&manager));
                }
                for data in ps.workspaces.values_mut() {
                    data.instances.retain(|i| i.data() != Some(&manager));
                }
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtWorkspaceManagerV1) {
        state
            .hecate
            .ext_workspace_state
            .instances
            .retain(|x, _| x != resource);
    }
}

impl Dispatch2<ExtWorkspaceHandleV1, State> for WrappedManager {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ExtWorkspaceHandleV1,
        request: ext_workspace_handle_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        let ps = &mut state.hecate.ext_workspace_state;
        let Some(id) = ps
            .workspaces
            .iter()
            .find(|(_, data)| data.instances.contains(resource))
            .map(|(id, _)| *id)
        else {
            return;
        };
        match request {
            ext_workspace_handle_v1::Request::Activate => {
                if let Some(actions) = ps.instances.get_mut(&self.0) {
                    actions.push(id);
                }
            }
            ext_workspace_handle_v1::Request::Deactivate => (),
            ext_workspace_handle_v1::Request::Assign { .. } => (),
            ext_workspace_handle_v1::Request::Remove => (),
            ext_workspace_handle_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtWorkspaceHandleV1) {
        for data in state.hecate.ext_workspace_state.workspaces.values_mut() {
            data.instances.retain(|i| i != resource);
        }
    }
}

impl Dispatch2<ExtWorkspaceGroupHandleV1, State> for WrappedManager {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _resource: &ExtWorkspaceGroupHandleV1,
        request: ext_workspace_group_handle_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_workspace_group_handle_v1::Request::CreateWorkspace { .. } => (),
            ext_workspace_group_handle_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ExtWorkspaceGroupHandleV1) {
        for data in state.hecate.ext_workspace_state.groups.values_mut() {
            data.instances.retain(|i| i != resource);
        }
    }
}
