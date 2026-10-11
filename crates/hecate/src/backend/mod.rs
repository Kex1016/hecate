//! Backends: real session (DRM/KMS), nested (winit), headless (tests).

pub mod gamma;
pub mod headless;
pub mod tty;
pub mod winit;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::input::keyboard::LedState;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::config::Config;
use crate::config::keys::Modifiers;
use crate::state::{Hecate, OutputState};

pub use headless::Headless;
pub use tty::Tty;
pub use winit::Winit;

#[allow(clippy::large_enum_variant)]
pub enum Backend {
    Tty(Tty),
    Winit(Winit),
    Headless(Headless),
}

impl Backend {
    pub fn init(&mut self, hecate: &mut Hecate) {
        match self {
            Backend::Tty(b) => b.init(hecate),
            Backend::Winit(b) => b.init(hecate),
            Backend::Headless(b) => b.init(hecate),
        }
    }

    pub fn seat_name(&self) -> String {
        match self {
            Backend::Tty(b) => b.seat_name(),
            Backend::Winit(_) => "winit".into(),
            Backend::Headless(_) => "headless".into(),
        }
    }

    /// `Mod` is Super on a real session and Alt when nested (so it doesn't fight the host).
    pub fn mod_key(&self) -> Modifiers {
        match self {
            Backend::Winit(_) => Modifiers::ALT,
            _ => Modifiers::SUPER,
        }
    }

    /// Render an output. Returns whether a frame was submitted.
    pub fn render(&mut self, hecate: &mut Hecate, output: &Output) -> bool {
        match self {
            Backend::Tty(b) => b.render(hecate, output),
            Backend::Winit(b) => b.render(hecate, output),
            Backend::Headless(b) => b.render(hecate, output),
        }
    }

    /// Whether submitted frames complete asynchronously (vblank events).
    pub fn waits_for_vblank(&self) -> bool {
        matches!(self, Backend::Tty(_))
    }

    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        match self {
            Backend::Tty(b) => b.import_dmabuf(dmabuf),
            Backend::Winit(b) => b.import_dmabuf(dmabuf),
            Backend::Headless(b) => b.import_dmabuf(dmabuf),
        }
    }

    pub fn early_import(&mut self, surface: &WlSurface) {
        if let Backend::Tty(b) = self {
            b.early_import(surface);
        }
    }

    pub fn update_led_state(&mut self, led: LedState) {
        if let Backend::Tty(b) = self {
            b.update_led_state(led);
        }
    }

    pub fn set_outputs_powered(&mut self, hecate: &mut Hecate, on: bool) {
        if let Backend::Tty(b) = self {
            b.set_outputs_powered(hecate, on);
        }
    }

    pub fn apply_output_config(&mut self, hecate: &mut Hecate) {
        match self {
            Backend::Tty(b) => b.apply_output_config(hecate),
            Backend::Winit(_) | Backend::Headless(_) => {
                // Only scale/transform/position apply to virtual outputs.
                let outputs: Vec<Output> = hecate.wm.outputs().cloned().collect();
                for o in outputs {
                    hecate.apply_output_props(&o);
                }
            }
        }
    }

    pub fn apply_input_config(&mut self, config: &Config) {
        if let Backend::Tty(b) = self {
            b.apply_input_config(config);
        }
    }

    pub fn on_device_added<D: smithay::backend::input::Device>(&mut self, device: &D, config: &Config) {
        if let Backend::Tty(b) = self {
            b.on_device_added(device, config);
        }
    }

    /// Gamma ramp length per channel, or None if gamma can't be set on this output.
    pub fn gamma_size(&self, output: &Output) -> Option<u32> {
        match self {
            Backend::Tty(b) => b.gamma_size(output),
            Backend::Winit(_) => None,
            Backend::Headless(b) => Some(b.gamma_size(output)),
        }
    }

    /// Set (Some) or reset (None) an output's gamma ramp: red, green, blue concatenated.
    pub fn set_gamma(&mut self, output: &Output, ramp: Option<Vec<u16>>) -> anyhow::Result<()> {
        match self {
            Backend::Tty(b) => b.set_gamma(output, ramp),
            Backend::Winit(_) => anyhow::bail!("gamma is not supported when nested"),
            Backend::Headless(b) => b.set_gamma(output, ramp),
        }
    }

    /// Run something with the primary renderer, if there is one.
    pub fn with_renderer<T>(
        &mut self,
        f: impl FnOnce(&mut smithay::backend::renderer::gles::GlesRenderer) -> T,
    ) -> Option<T> {
        match self {
            Backend::Tty(b) => b.renderer_mut().map(f),
            Backend::Winit(b) => Some(f(b.renderer())),
            Backend::Headless(b) => b.renderer().map(f),
        }
    }

    pub fn headless(&mut self) -> &mut Headless {
        match self {
            Backend::Headless(h) => h,
            _ => panic!("backend is not headless"),
        }
    }
}

impl Hecate {
    /// Register a new output: global, layout position, workspaces, render state.
    pub fn add_output(&mut self, output: Output) {
        output.create_global::<crate::state::State>(&self.display_handle);
        let (background, lock_background) = crate::render::new_output_buffers();
        self.output_state.insert(
            output.clone(),
            OutputState {
                background,
                lock_background,
                ..Default::default()
            },
        );
        self.apply_output_props(&output);
        if output.current_location() == (0, 0).into()
            && self
                .config
                .outputs
                .get(&output.name())
                .and_then(|o| o.position)
                .is_none()
        {
            // Place to the right of existing outputs.
            let x = self
                .wm
                .outputs()
                .map(|o| {
                    let g = crate::wm::Wm::output_geometry(o);
                    g.loc.x + g.size.w
                })
                .max()
                .unwrap_or(0);
            output.change_current_state(None, None, None, Some((x, 0).into()));
        }
        let config = self.config.clone();
        self.wm.add_output(output.clone(), &config);
        if self
            .config
            .outputs
            .get(&output.name())
            .is_some_and(|o| o.focus_at_startup)
        {
            self.wm.focused_output = Some(output.clone());
        }
        self.update_output_buffers(&output);
        if self.wm.outputs().count() == 1 {
            let center = crate::wm::geometry::center(crate::wm::Wm::output_geometry(&output));
            self.pointer_location = center.to_f64();
        }
        self.queue_redraw(&output);
    }

    pub fn remove_output(&mut self, output: &Output) {
        self.gamma_control_state.output_removed(output);
        self.wm.remove_output(output);
        self.output_state.remove(output);
        smithay::desktop::layer_map_for_output(output).cleanup();
        self.queue_redraw_all();
    }

    /// Apply scale, transform and position from the config to an output.
    pub fn apply_output_props(&mut self, output: &Output) {
        let Some(cfg) = self.config.outputs.get(&output.name()).cloned() else {
            return;
        };
        let scale = cfg.scale.map(smithay::output::Scale::Fractional);
        let transform = cfg.transform.map(Into::into);
        let position = cfg.position.map(|[x, y]| (x, y).into());
        output.change_current_state(None, transform, scale, position);
        self.update_output_buffers(output);
        smithay::desktop::layer_map_for_output(output).arrange();
    }
}
