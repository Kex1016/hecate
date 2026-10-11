//! Real session backend: DRM/KMS outputs, libinput, libseat.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::input::keyboard::LedState;
use smithay::output::Output;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::config::Config;
use crate::state::{Hecate, State};

pub struct Tty {
    seat_name: String,
}

impl Tty {
    pub fn new(_event_loop: LoopHandle<'static, State>) -> anyhow::Result<Self> {
        anyhow::bail!("the DRM backend is not implemented yet")
    }

    pub fn init(&mut self, _hecate: &mut Hecate) {}

    pub fn seat_name(&self) -> String {
        self.seat_name.clone()
    }

    pub fn render(&mut self, _hecate: &mut Hecate, _output: &Output) -> bool {
        false
    }

    pub fn import_dmabuf(&mut self, _dmabuf: &Dmabuf) -> bool {
        false
    }

    pub fn early_import(&mut self, _surface: &WlSurface) {}

    pub fn update_led_state(&mut self, _led: LedState) {}

    pub fn set_outputs_powered(&mut self, _hecate: &mut Hecate, _on: bool) {}

    pub fn apply_output_config(&mut self, _hecate: &mut Hecate) {}

    pub fn apply_input_config(&mut self, _config: &Config) {}

    pub fn on_device_added<D: smithay::backend::input::Device>(&mut self, _device: &D, _config: &Config) {}
}
