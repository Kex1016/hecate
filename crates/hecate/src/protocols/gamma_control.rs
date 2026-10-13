//! wlr-gamma-control-unstable-v1 (night light tools: wlsunset, gammastep, hyprsunset-likes).
//!
//! Adapted from niri (src/protocols/gamma_control.rs), GPL-3.0-or-later.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::server::zwlr_gamma_control_manager_v1::{
    self, ZwlrGammaControlManagerV1,
};
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::server::zwlr_gamma_control_v1::{
    self, ZwlrGammaControlV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use super::EmptyData;
use crate::state::State;

const VERSION: u32 = 1;

pub struct GammaControlManagerState {
    /// Active gamma controls only; failed ones are removed.
    gamma_controls: HashMap<Output, ZwlrGammaControlV1>,
}

pub struct GammaControlManagerGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

pub struct GammaControlState {
    gamma_size: u32,
}

impl GammaControlManagerState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let global_data = GammaControlManagerGlobalData {
            filter: Box::new(filter),
        };
        display.create_global::<State, ZwlrGammaControlManagerV1, _>(VERSION, global_data);
        Self {
            gamma_controls: HashMap::new(),
        }
    }

    pub fn output_removed(&mut self, output: &Output) {
        if let Some(control) = self.gamma_controls.remove(output) {
            control.failed();
        }
    }

    /// Outputs with a client-set gamma ramp.
    pub fn controlled_outputs(&self) -> impl Iterator<Item = &Output> {
        self.gamma_controls.keys()
    }
}

impl GlobalDispatch2<ZwlrGammaControlManagerV1, State> for GammaControlManagerGlobalData {
    fn bind(
        &self,
        _state: &mut State,
        _handle: &DisplayHandle,
        _client: &Client,
        manager: New<ZwlrGammaControlManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(manager, EmptyData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ZwlrGammaControlManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &ZwlrGammaControlManagerV1,
        request: zwlr_gamma_control_manager_v1::Request,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_gamma_control_manager_v1::Request::GetGammaControl { id, output } => {
                // Only one client may control an output's gamma at a time.
                if let Some(output) = Output::from_resource(&output)
                    && !state
                        .hecate
                        .gamma_control_state
                        .gamma_controls
                        .contains_key(&output)
                    && let Some(gamma_size) = state.backend.gamma_size(&output).filter(|s| *s > 0)
                {
                    let control = data_init.init(id, GammaControlState { gamma_size });
                    control.gamma_size(gamma_size);
                    state
                        .hecate
                        .gamma_control_state
                        .gamma_controls
                        .insert(output, control);
                    return;
                }
                data_init.init(id, GammaControlState { gamma_size: 0 }).failed();
            }
            zwlr_gamma_control_manager_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }
}

fn read_ramp(fd: std::os::fd::OwnedFd, gamma_size: u32) -> anyhow::Result<Vec<u16>> {
    let mut bytes = vec![0u8; gamma_size as usize * 3 * 2];
    let mut file = File::from(fd);
    file.read_exact(&mut bytes)?;
    // There must be no more data.
    anyhow::ensure!(file.read(&mut [0])? == 0, "gamma data is too large");
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes(*c))
        .collect())
}

impl Dispatch2<ZwlrGammaControlV1, State> for GammaControlState {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        resource: &ZwlrGammaControlV1,
        request: zwlr_gamma_control_v1::Request,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            zwlr_gamma_control_v1::Request::SetGamma { fd } => {
                let controls = &mut state.hecate.gamma_control_state.gamma_controls;
                let Some(output) = controls
                    .iter()
                    .find(|(_, x)| *x == resource)
                    .map(|(o, _)| o.clone())
                else {
                    return;
                };
                let res = read_ramp(fd, self.gamma_size)
                    .and_then(|ramp| state.backend.set_gamma(&output, Some(ramp)));
                if let Err(err) = res {
                    tracing::warn!("gamma control on {} failed: {err:?}", output.name());
                    resource.failed();
                    state.hecate.gamma_control_state.gamma_controls.remove(&output);
                    let _ = state.backend.set_gamma(&output, None);
                }
            }
            zwlr_gamma_control_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, resource: &ZwlrGammaControlV1) {
        let controls = &mut state.hecate.gamma_control_state.gamma_controls;
        let Some(output) = controls
            .iter()
            .find(|(_, x)| *x == resource)
            .map(|(o, _)| o.clone())
        else {
            return;
        };
        controls.remove(&output);
        // Gamma goes back to normal when the client lets go.
        let _ = state.backend.set_gamma(&output, None);
    }
}
