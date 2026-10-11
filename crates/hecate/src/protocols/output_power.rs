//! wlr-output-power-management-unstable-v1 (wlopm, swayidle-style DPMS from shells).

use std::collections::HashMap;

use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::server::zwlr_output_power_manager_v1::{
    self, ZwlrOutputPowerManagerV1,
};
use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::server::zwlr_output_power_v1::{
    self, Mode, ZwlrOutputPowerV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource, WEnum};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;

const VERSION: u32 = 1;

pub struct OutputPowerState {
    controls: HashMap<Output, Vec<ZwlrOutputPowerV1>>,
}

pub struct OutputPowerGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

/// User data of a power control: its output, or None if it failed at creation.
pub struct PowerControlData(Option<Output>);

fn mode(on: bool) -> Mode {
    if on { Mode::On } else { Mode::Off }
}

impl OutputPowerState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        display.create_global::<State, ZwlrOutputPowerManagerV1, _>(
            VERSION,
            OutputPowerGlobalData {
                filter: Box::new(filter),
            },
        );
        Self {
            controls: HashMap::new(),
        }
    }

    /// Tell every control of `output` about its new mode.
    pub fn mode_changed(&self, output: &Output, on: bool) {
        for c in self.controls.get(output).into_iter().flatten() {
            c.mode(mode(on));
        }
    }

    pub fn output_removed(&mut self, output: &Output) {
        for c in self.controls.remove(output).into_iter().flatten() {
            c.failed();
        }
    }
}

impl GlobalDispatch2<ZwlrOutputPowerManagerV1, State> for OutputPowerGlobalData {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrOutputPowerManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, EmptyData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ZwlrOutputPowerManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &ZwlrOutputPowerManagerV1,
        request: zwlr_output_power_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_output_power_manager_v1::Request::GetOutputPower { id, output } => {
                let output =
                    Output::from_resource(&output).filter(|o| state.hecate.output_state.contains_key(o));
                let Some(output) = output else {
                    data_init.init(id, PowerControlData(None)).failed();
                    return;
                };
                let control = data_init.init(id, PowerControlData(Some(output.clone())));
                let on = !state.hecate.output_state[&output].powered_off;
                control.mode(mode(on));
                state
                    .hecate
                    .output_power_state
                    .controls
                    .entry(output)
                    .or_default()
                    .push(control);
            }
            zwlr_output_power_manager_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch2<ZwlrOutputPowerV1, State> for PowerControlData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ZwlrOutputPowerV1,
        request: zwlr_output_power_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_output_power_v1::Request::SetMode { mode } => {
                let Some(output) = &self.0 else {
                    return;
                };
                let on = match mode {
                    WEnum::Value(Mode::On) => true,
                    WEnum::Value(Mode::Off) => false,
                    _ => {
                        resource.post_error(zwlr_output_power_v1::Error::InvalidMode, "unknown power mode");
                        return;
                    }
                };
                if !state.hecate.output_state.contains_key(output) {
                    return;
                }
                let State { backend, hecate } = state;
                backend.set_output_powered(hecate, output, on);
            }
            zwlr_output_power_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ZwlrOutputPowerV1) {
        if let Some(output) = &self.0
            && let Some(list) = state.hecate.output_power_state.controls.get_mut(output)
        {
            list.retain(|c| c != resource);
        }
    }
}
