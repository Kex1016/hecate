//! wlr-output-management-unstable-v1 (kanshi, wdisplays, nwg-displays, wlr-randr, shikane).
//!
//! Adapted from niri (src/protocols/output_management.rs), GPL-3.0-or-later.
//!
//! Applied configurations become runtime overrides under `outputs.<name>`, so
//! `hecate msg get outputs` shows them and `hecate msg unset` reverts them.

use std::collections::HashMap;
use std::mem;

use serde_json::{Value, json};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_configuration_head_v1::{
    self, ZwlrOutputConfigurationHeadV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_configuration_v1::{
    self, ZwlrOutputConfigurationV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_head_v1::{
    self, AdaptiveSyncState, ZwlrOutputHeadV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_manager_v1::{
    self, ZwlrOutputManagerV1,
};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::server::zwlr_output_mode_v1::{
    self, ZwlrOutputModeV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_output::Transform as WlTransform;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource, WEnum};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::backend::{HeadInfo, mode_string};
use crate::state::State;

const VERSION: u32 = 4;

pub struct OutputManagementState {
    display: DisplayHandle,
    serial: u32,
    clients: HashMap<ClientId, ClientData>,
    current: Vec<HeadInfo>,
}

struct ClientData {
    manager: ZwlrOutputManagerV1,
    heads: HashMap<String, (ZwlrOutputHeadV1, Vec<ZwlrOutputModeV1>)>,
    confs: HashMap<ZwlrOutputConfigurationV1, ConfState>,
}

enum ConfState {
    Ongoing(HashMap<String, PendingHead>),
    Finished,
}

#[derive(Debug, Default, Clone)]
struct PendingHead {
    enabled: bool,
    /// (width, height, refresh mHz)
    mode: Option<(i32, i32, i32)>,
    position: Option<(i32, i32)>,
    scale: Option<f64>,
    transform: Option<&'static str>,
    vrr: Option<bool>,
}

pub struct OutputManagementGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

pub struct ConfigurationData {
    serial: u32,
}

/// User data of a configuration head: the head name and its configuration, or cancelled.
pub enum ConfigurationHeadData {
    Cancelled,
    Ok(String, ZwlrOutputConfigurationV1),
}

/// User data of a head: the output name.
pub struct HeadName(String);

impl OutputManagementState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let global_data = OutputManagementGlobalData {
            filter: Box::new(filter),
        };
        display.create_global::<State, ZwlrOutputManagerV1, _>(VERSION, global_data);
        Self {
            display: display.clone(),
            serial: 0,
            clients: HashMap::new(),
            current: Vec::new(),
        }
    }
}

/// Resend heads if outputs changed. Called once per loop iteration.
pub fn refresh(state: &mut State) {
    if !state.hecate.output_heads_dirty {
        return;
    }
    state.hecate.output_heads_dirty = false;
    let new = state.backend.heads(&state.hecate);
    let ps = &mut state.hecate.output_management_state;
    if new == ps.current {
        return;
    }

    let old_heads = mem::take(&mut ps.current);
    for old in &old_heads {
        match new.iter().find(|h| h.name == old.name) {
            None => remove_head(ps, &old.name),
            // A changed mode list is simplest to announce as a new head.
            Some(h) if h.modes != old.modes => remove_head(ps, &old.name),
            Some(h) => update_head(ps, old, h),
        }
    }
    for head in &new {
        let existed = old_heads
            .iter()
            .any(|old| old.name == head.name && old.modes == head.modes);
        if !existed {
            for data in ps.clients.values_mut() {
                if let Some(client) = data.manager.client() {
                    send_new_head(&ps.display, &client, data, head);
                }
            }
        }
    }

    ps.current = new;
    ps.serial += 1;
    for data in ps.clients.values_mut() {
        data.manager.done(ps.serial);
        // In-progress configurations refer to the old state.
        for (conf, st) in data.confs.iter_mut() {
            if matches!(st, ConfState::Ongoing(_)) {
                conf.cancelled();
                *st = ConfState::Finished;
            }
        }
    }
}

fn remove_head(ps: &mut OutputManagementState, name: &str) {
    for data in ps.clients.values_mut() {
        if let Some((head, modes)) = data.heads.remove(name) {
            for m in modes {
                m.finished();
            }
            head.finished();
        }
    }
}

fn update_head(ps: &OutputManagementState, old: &HeadInfo, new: &HeadInfo) {
    for data in ps.clients.values() {
        let Some((head, modes)) = data.heads.get(&new.name) else {
            continue;
        };
        match (old.current, new.current) {
            (_, None) => {
                if old.current.is_some() {
                    head.enabled(0);
                }
            }
            (o, Some(n)) => {
                if o.is_none() {
                    head.enabled(1);
                }
                if (o.map(|o| o.mode) != Some(n.mode))
                    && let Some(m) = n.mode.and_then(|i| modes.get(i))
                {
                    head.current_mode(m);
                }
                if o.map(|o| o.position) != Some(n.position) {
                    head.position(n.position.0, n.position.1);
                }
                if o.map(|o| o.scale) != Some(n.scale) {
                    head.scale(n.scale);
                }
                if o.map(|o| o.transform) != Some(n.transform) {
                    head.transform(n.transform.into());
                }
                if o.map(|o| o.vrr) != Some(n.vrr)
                    && head.version() >= zwlr_output_head_v1::EVT_ADAPTIVE_SYNC_SINCE
                {
                    head.adaptive_sync(vrr_state(n.vrr));
                }
            }
        }
    }
}

fn vrr_state(on: bool) -> AdaptiveSyncState {
    if on {
        AdaptiveSyncState::Enabled
    } else {
        AdaptiveSyncState::Disabled
    }
}

fn send_new_head(display: &DisplayHandle, client: &Client, data: &mut ClientData, info: &HeadInfo) {
    let Ok(head) = client.create_resource::<ZwlrOutputHeadV1, _, State>(
        display,
        data.manager.version(),
        HeadName(info.name.clone()),
    ) else {
        return;
    };
    data.manager.head(&head);
    head.name(info.name.clone());
    head.description(format!("{} - {} - {}", info.make, info.model, info.name));
    if info.physical_size.0 > 0 && info.physical_size.1 > 0 {
        head.physical_size(info.physical_size.0, info.physical_size.1);
    }
    let mut modes = Vec::with_capacity(info.modes.len());
    for (i, m) in info.modes.iter().enumerate() {
        let Ok(mode) =
            client.create_resource::<ZwlrOutputModeV1, _, State>(display, head.version(), EmptyData)
        else {
            continue;
        };
        head.mode(&mode);
        mode.size(m.width, m.height);
        if m.refresh > 0 {
            mode.refresh(m.refresh);
        }
        if m.preferred {
            mode.preferred();
        }
        if info.current.and_then(|c| c.mode) == Some(i) {
            head.current_mode(&mode);
        }
        modes.push(mode);
    }
    head.enabled(info.current.is_some() as i32);
    if let Some(c) = info.current {
        head.position(c.position.0, c.position.1);
        head.transform(c.transform.into());
        head.scale(c.scale);
    }
    if head.version() >= zwlr_output_head_v1::EVT_MAKE_SINCE {
        head.make(info.make.clone());
    }
    if head.version() >= zwlr_output_head_v1::EVT_MODEL_SINCE {
        head.model(info.model.clone());
    }
    if head.version() >= zwlr_output_head_v1::EVT_SERIAL_NUMBER_SINCE {
        head.serial_number(info.serial.clone());
    }
    if head.version() >= zwlr_output_head_v1::EVT_ADAPTIVE_SYNC_SINCE {
        head.adaptive_sync(vrr_state(info.current.is_some_and(|c| c.vrr)));
    }
    data.heads.insert(info.name.clone(), (head, modes));
}

/// Overrides for a configuration, or why it can't be applied.
fn to_overrides(
    state: &State,
    pending: &HashMap<String, PendingHead>,
) -> Result<Vec<(String, Value)>, String> {
    let heads = &state.hecate.output_management_state.current;
    // Heads the configuration doesn't mention stay as they are.
    let any_enabled = heads.iter().any(|h| match pending.get(&h.name) {
        Some(p) => p.enabled,
        None => h.current.is_some(),
    });
    if !any_enabled {
        return Err("configuration would disable every output".into());
    }
    let mut out = Vec::new();
    for (name, p) in pending {
        let info = heads
            .iter()
            .find(|h| h.name == *name)
            .ok_or_else(|| format!("unknown output {name}"))?;
        let base = format!("outputs.{name}");
        out.push((format!("{base}.enable"), json!(p.enabled)));
        if !p.enabled {
            continue;
        }
        if let Some((w, h, r)) = p.mode {
            // Modelines aren't supported; a custom mode has to match a real mode's size.
            if !info.modes.iter().any(|m| m.width == w && m.height == h) {
                return Err(format!("{name} has no {w}x{h} mode"));
            }
            let r = if r > 0 {
                r
            } else {
                info.modes
                    .iter()
                    .filter(|m| m.width == w && m.height == h)
                    .map(|m| m.refresh)
                    .max()
                    .unwrap_or(0)
            };
            out.push((format!("{base}.mode"), json!(mode_string(w, h, r))));
        }
        if let Some((x, y)) = p.position {
            out.push((format!("{base}.position"), json!([x, y])));
        }
        if let Some(s) = p.scale {
            out.push((format!("{base}.scale"), json!(s)));
        }
        if let Some(t) = p.transform {
            out.push((format!("{base}.transform"), json!(t)));
        }
        if let Some(v) = p.vrr {
            let current = state
                .hecate
                .config
                .outputs
                .get(name)
                .and_then(|o| o.adaptive_sync)
                .unwrap_or_default();
            let new = match (v, current) {
                (false, _) => "off",
                (true, crate::config::Vrr::Fullscreen) => "fullscreen",
                (true, _) => "on",
            };
            out.push((format!("{base}.adaptive_sync"), json!(new)));
        }
    }
    Ok(out)
}

impl GlobalDispatch2<ZwlrOutputManagerV1, State> for OutputManagementGlobalData {
    fn bind(
        &self,
        state: &mut State,
        display: &DisplayHandle,
        client: &Client,
        manager: New<ZwlrOutputManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(manager, EmptyData);
        // Make sure the head list is current before the client sees it.
        if state.hecate.output_heads_dirty {
            refresh(state);
        }
        let ps = &mut state.hecate.output_management_state;
        let mut data = ClientData {
            manager: manager.clone(),
            heads: HashMap::new(),
            confs: HashMap::new(),
        };
        for info in &ps.current {
            send_new_head(display, client, &mut data, info);
        }
        ps.clients.insert(client.id(), data);
        manager.done(ps.serial);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ZwlrOutputManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        client: &Client,
        _manager: &ZwlrOutputManagerV1,
        request: zwlr_output_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        let ps = &mut state.hecate.output_management_state;
        match request {
            zwlr_output_manager_v1::Request::CreateConfiguration { id, serial } => {
                let conf = data_init.init(id, ConfigurationData { serial });
                if serial != ps.serial {
                    conf.cancelled();
                }
                if let Some(data) = ps.clients.get_mut(&client.id()) {
                    data.confs.insert(conf, ConfState::Ongoing(HashMap::new()));
                }
            }
            zwlr_output_manager_v1::Request::Stop => {
                if let Some(c) = ps.clients.remove(&client.id()) {
                    c.manager.finished();
                }
            }
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, client: ClientId, _resource: &ZwlrOutputManagerV1) {
        state.hecate.output_management_state.clients.remove(&client);
    }
}

impl Dispatch2<ZwlrOutputConfigurationV1, State> for ConfigurationData {
    fn request(
        &self,
        state: &mut State,
        client: &Client,
        conf: &ZwlrOutputConfigurationV1,
        request: zwlr_output_configuration_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        let ps = &mut state.hecate.output_management_state;
        let outdated = self.serial != ps.serial;
        let pending = ps
            .clients
            .get_mut(&client.id())
            .and_then(|d| d.confs.get_mut(conf));

        match request {
            zwlr_output_configuration_v1::Request::EnableHead { id, head } => {
                let name = head.data::<HeadName>().map(|h| h.0.clone());
                let (Some(name), false, Some(pending)) = (name, outdated, pending) else {
                    data_init.init(id, ConfigurationHeadData::Cancelled);
                    return;
                };
                let ConfState::Ongoing(heads) = pending else {
                    data_init.init(id, ConfigurationHeadData::Cancelled);
                    conf.post_error(
                        zwlr_output_configuration_v1::Error::AlreadyUsed,
                        "configuration has already been used",
                    );
                    return;
                };
                if heads.contains_key(&name) {
                    data_init.init(id, ConfigurationHeadData::Cancelled);
                    conf.post_error(
                        zwlr_output_configuration_v1::Error::AlreadyConfiguredHead,
                        "head has already been configured",
                    );
                    return;
                }
                heads.insert(
                    name.clone(),
                    PendingHead {
                        enabled: true,
                        ..Default::default()
                    },
                );
                data_init.init(id, ConfigurationHeadData::Ok(name, conf.clone()));
            }
            zwlr_output_configuration_v1::Request::DisableHead { head } => {
                let name = head.data::<HeadName>().map(|h| h.0.clone());
                let (Some(name), false, Some(pending)) = (name, outdated, pending) else {
                    return;
                };
                let ConfState::Ongoing(heads) = pending else {
                    conf.post_error(
                        zwlr_output_configuration_v1::Error::AlreadyUsed,
                        "configuration has already been used",
                    );
                    return;
                };
                if heads.contains_key(&name) {
                    conf.post_error(
                        zwlr_output_configuration_v1::Error::AlreadyConfiguredHead,
                        "head has already been configured",
                    );
                    return;
                }
                heads.insert(name, PendingHead::default());
            }
            request @ (zwlr_output_configuration_v1::Request::Apply
            | zwlr_output_configuration_v1::Request::Test) => {
                let dry_run = matches!(request, zwlr_output_configuration_v1::Request::Test);
                if outdated {
                    conf.cancelled();
                    return;
                }
                let Some(pending) = pending else {
                    return;
                };
                let ConfState::Ongoing(heads) = mem::replace(pending, ConfState::Finished) else {
                    conf.post_error(
                        zwlr_output_configuration_v1::Error::AlreadyUsed,
                        "configuration has already been used",
                    );
                    return;
                };
                let result = to_overrides(state, &heads).and_then(|o| state.set_overrides(o, dry_run));
                match result {
                    Ok(()) => conf.succeeded(),
                    Err(err) => {
                        tracing::info!("output configuration rejected: {err}");
                        conf.failed();
                    }
                }
            }
            zwlr_output_configuration_v1::Request::Destroy => {
                if let Some(d) = state.hecate.output_management_state.clients.get_mut(&client.id()) {
                    d.confs.remove(conf);
                }
            }
            _ => unreachable!(),
        }
    }
}

fn transform_name(t: WlTransform) -> Option<&'static str> {
    Some(match t {
        WlTransform::Normal => "normal",
        WlTransform::_90 => "90",
        WlTransform::_180 => "180",
        WlTransform::_270 => "270",
        WlTransform::Flipped => "flipped",
        WlTransform::Flipped90 => "flipped-90",
        WlTransform::Flipped180 => "flipped-180",
        WlTransform::Flipped270 => "flipped-270",
        _ => return None,
    })
}

impl Dispatch2<ZwlrOutputConfigurationHeadV1, State> for ConfigurationHeadData {
    fn request(
        &self,
        state: &mut State,
        client: &Client,
        conf_head: &ZwlrOutputConfigurationHeadV1,
        request: zwlr_output_configuration_head_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        let ConfigurationHeadData::Ok(name, conf) = self else {
            return;
        };
        let ps = &mut state.hecate.output_management_state;
        if conf
            .data::<ConfigurationData>()
            .is_none_or(|d| d.serial != ps.serial)
        {
            return;
        }
        let Some(data) = ps.clients.get_mut(&client.id()) else {
            return;
        };
        let modes = data.heads.get(name).map(|(_, m)| m.clone()).unwrap_or_default();
        let Some(ConfState::Ongoing(heads)) = data.confs.get_mut(conf) else {
            conf.post_error(
                zwlr_output_configuration_v1::Error::AlreadyUsed,
                "configuration has already been used",
            );
            return;
        };
        let Some(pending) = heads.get_mut(name) else {
            return;
        };

        match request {
            zwlr_output_configuration_head_v1::Request::SetMode { mode } => {
                let info = ps.current.iter().find(|h| h.name == *name);
                let Some(m) = modes
                    .iter()
                    .position(|m| m.id() == mode.id())
                    .and_then(|i| info?.modes.get(i))
                else {
                    conf_head.post_error(
                        zwlr_output_configuration_head_v1::Error::InvalidMode,
                        "unknown mode",
                    );
                    return;
                };
                if pending.mode.is_some() {
                    conf_head.post_error(
                        zwlr_output_configuration_head_v1::Error::AlreadySet,
                        "mode has already been set",
                    );
                    return;
                }
                pending.mode = Some((m.width, m.height, m.refresh));
            }
            zwlr_output_configuration_head_v1::Request::SetCustomMode {
                width,
                height,
                refresh,
            } => {
                if width <= 0 || height <= 0 || refresh < 0 {
                    conf_head.post_error(
                        zwlr_output_configuration_head_v1::Error::InvalidCustomMode,
                        "invalid custom mode",
                    );
                    return;
                }
                pending.mode = Some((width, height, refresh));
            }
            zwlr_output_configuration_head_v1::Request::SetPosition { x, y } => {
                pending.position = Some((x, y));
            }
            zwlr_output_configuration_head_v1::Request::SetTransform { transform } => {
                let Some(t) = transform.into_result().ok().and_then(transform_name) else {
                    conf_head.post_error(
                        zwlr_output_configuration_head_v1::Error::InvalidTransform,
                        "unknown transform",
                    );
                    return;
                };
                pending.transform = Some(t);
            }
            zwlr_output_configuration_head_v1::Request::SetScale { scale } => {
                if scale <= 0. || !scale.is_finite() {
                    conf_head.post_error(
                        zwlr_output_configuration_head_v1::Error::InvalidScale,
                        "scale must be positive",
                    );
                    return;
                }
                pending.scale = Some(scale);
            }
            zwlr_output_configuration_head_v1::Request::SetAdaptiveSync { state } => {
                pending.vrr = match state {
                    WEnum::Value(AdaptiveSyncState::Enabled) => Some(true),
                    WEnum::Value(AdaptiveSyncState::Disabled) => Some(false),
                    _ => {
                        conf_head.post_error(
                            zwlr_output_configuration_head_v1::Error::InvalidAdaptiveSyncState,
                            "unknown adaptive sync state",
                        );
                        return;
                    }
                };
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch2<ZwlrOutputHeadV1, State> for HeadName {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _head: &ZwlrOutputHeadV1,
        request: zwlr_output_head_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_output_head_v1::Request::Release => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, client: ClientId, resource: &ZwlrOutputHeadV1) {
        if let Some(c) = state.hecate.output_management_state.clients.get_mut(&client)
            && c.heads.get(&self.0).is_some_and(|(h, _)| h == resource)
        {
            c.heads.remove(&self.0);
        }
    }
}

impl Dispatch2<ZwlrOutputModeV1, State> for EmptyData {
    fn request(
        &self,
        _state: &mut State,
        _client: &Client,
        _mode: &ZwlrOutputModeV1,
        request: zwlr_output_mode_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_output_mode_v1::Request::Release => (),
            _ => unreachable!(),
        }
    }
}
