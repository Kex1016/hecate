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

/// A connected monitor as output-management sees it, enabled or not.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadInfo {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    /// Millimetres.
    pub physical_size: (i32, i32),
    pub modes: Vec<HeadMode>,
    /// Set when the output is enabled.
    pub current: Option<HeadCurrent>,
    pub vrr_supported: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadMode {
    pub width: i32,
    pub height: i32,
    /// mHz.
    pub refresh: i32,
    pub preferred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadCurrent {
    /// Index into `modes`.
    pub mode: Option<usize>,
    pub position: (i32, i32),
    pub scale: f64,
    pub transform: smithay::utils::Transform,
    pub vrr: bool,
}

impl HeadInfo {
    /// Head info for an enabled smithay output.
    pub fn from_output(output: &Output, config: &Config, vrr_supported: bool) -> Self {
        let props = output.physical_properties();
        let current_mode = output.current_mode();
        let preferred = output.preferred_mode();
        let mut all_modes = output.modes();
        if let Some(c) = current_mode
            && !all_modes.contains(&c)
        {
            all_modes.push(c);
        }
        let modes: Vec<HeadMode> = all_modes
            .iter()
            .map(|m| HeadMode {
                width: m.size.w,
                height: m.size.h,
                refresh: m.refresh,
                preferred: Some(*m) == preferred,
            })
            .collect();
        let mode = current_mode.and_then(|c| all_modes.iter().position(|m| *m == c));
        let loc = output.current_location();
        let vrr = config
            .outputs
            .get(&output.name())
            .and_then(|o| o.adaptive_sync)
            .is_some_and(|v| v != crate::config::Vrr::Off);
        HeadInfo {
            name: output.name(),
            make: props.make,
            model: props.model,
            serial: props.serial_number,
            physical_size: (props.size.w, props.size.h),
            modes,
            current: Some(HeadCurrent {
                mode,
                position: (loc.x, loc.y),
                scale: output.current_scale().fractional_scale(),
                transform: output.current_transform(),
                vrr: vrr && vrr_supported,
            }),
            vrr_supported,
        }
    }
}

/// Pick a mode for a config `mode` string from `(width, height, refresh mHz)` triples.
/// Falls back to `preferred` when unset or nothing matches the size.
pub fn pick_mode_index(modes: &[(i32, i32, i32)], preferred: usize, wanted: Option<&str>) -> usize {
    let Some((w, h, refresh)) = wanted.and_then(crate::config::load::parse_mode) else {
        return preferred;
    };
    let candidates = modes.iter().enumerate().filter(|(_, m)| m.0 == w && m.1 == h);
    match refresh {
        Some(hz) => candidates
            .min_by_key(|(_, m)| ((m.2 as f64 / 1000.0 - hz).abs() * 1000.0) as i64)
            .map(|(i, _)| i)
            .unwrap_or(preferred),
        None => candidates
            .max_by_key(|(_, m)| m.2)
            .map(|(i, _)| i)
            .unwrap_or(preferred),
    }
}

/// Config `mode` string for a mode, exact enough to pick it again.
pub fn mode_string(width: i32, height: i32, refresh_mhz: i32) -> String {
    format!("{width}x{height}@{:.3}", refresh_mhz as f64 / 1000.0)
}

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
        let outputs: Vec<Output> = hecate.wm.outputs().cloned().collect();
        for o in outputs {
            self.set_output_powered(hecate, &o, on);
        }
    }

    /// Power an output off (DPMS) or on, and tell output-power clients.
    pub fn set_output_powered(&mut self, hecate: &mut Hecate, output: &Output, on: bool) {
        let Some(st) = hecate.output_state.get_mut(output) else {
            return;
        };
        if st.powered_off == !on {
            return;
        }
        st.powered_off = !on;
        if let Backend::Tty(b) = self {
            b.set_output_powered(hecate, output, on);
        }
        hecate.output_power_state.mode_changed(output, on);
        if on {
            hecate.queue_redraw(output);
        }
    }

    pub fn apply_output_config(&mut self, hecate: &mut Hecate) {
        hecate.output_heads_dirty = true;
        match self {
            Backend::Tty(b) => b.apply_output_config(hecate),
            Backend::Headless(b) => b.apply_output_config(hecate),
            Backend::Winit(_) => {
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

    /// Whether the output's connector can do adaptive sync.
    pub fn vrr_supported(&self, output: &Output) -> bool {
        match self {
            Backend::Tty(b) => b.vrr_supported(output),
            Backend::Headless(_) => true,
            Backend::Winit(_) => false,
        }
    }

    pub fn set_vrr(&mut self, output: &Output, on: bool) {
        match self {
            Backend::Tty(b) => b.set_vrr(output, on),
            Backend::Headless(b) => {
                b.vrr.insert(output.name(), on);
            }
            Backend::Winit(_) => (),
        }
    }

    /// Render node of the primary GPU, for dmabuf capture constraints.
    pub fn render_node(&self) -> Option<smithay::backend::drm::DrmNode> {
        match self {
            Backend::Tty(b) => Some(b.render_node()),
            _ => None,
        }
    }

    /// All connected monitors, including disabled ones.
    pub fn heads(&self, hecate: &Hecate) -> Vec<HeadInfo> {
        match self {
            Backend::Tty(b) => b.heads(hecate),
            Backend::Winit(_) => hecate
                .wm
                .outputs()
                .map(|o| HeadInfo::from_output(o, &hecate.config, false))
                .collect(),
            Backend::Headless(b) => b.heads(hecate),
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
        let global = output.create_global::<crate::state::State>(&self.display_handle);
        let (background, lock_background) = crate::render::new_output_buffers();
        self.output_state.insert(
            output.clone(),
            OutputState {
                background,
                lock_background,
                global: Some(global),
                ..Default::default()
            },
        );
        self.output_heads_dirty = true;
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
        self.output_power_state.output_removed(output);
        self.screencopy_state.remove_output(output);
        self.wm.remove_output(output);
        if let Some(global) = self.output_state.remove(output).and_then(|s| s.global) {
            // Disable first so clients stop binding, remove once they had time to notice.
            self.display_handle
                .disable_global::<crate::state::State>(global.clone());
            let dh = self.display_handle.clone();
            let timer =
                smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_secs(10));
            let _ = self.event_loop.insert_source(timer, move |_, _, _| {
                dh.remove_global::<crate::state::State>(global.clone());
                smithay::reexports::calloop::timer::TimeoutAction::Drop
            });
        }
        self.output_heads_dirty = true;
        // Layer surfaces belong to one output; clients recreate them elsewhere.
        let layers: Vec<_> = smithay::desktop::layer_map_for_output(output)
            .layers()
            .cloned()
            .collect();
        for layer in layers {
            layer.layer_surface().send_close();
            smithay::desktop::layer_map_for_output(output).unmap_layer(&layer);
        }
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
        self.output_heads_dirty = true;
        self.update_output_buffers(output);
        smithay::desktop::layer_map_for_output(output).arrange();
    }
}
