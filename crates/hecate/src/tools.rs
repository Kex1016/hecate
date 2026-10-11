//! Support for the CLI's window tools: identify, colour picking, gaming status.

use std::time::{Duration, Instant};

use hecate_ipc as ipc;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, Offscreen};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::state::{Hecate, State};
use crate::wm::{WindowExt, Wm};

impl State {
    /// Highlight every visible window's border for `ms` milliseconds.
    pub fn identify_windows(&mut self, ms: u32) {
        let ms = ms.clamp(100, 10_000);
        self.hecate.identify_until = Some(Instant::now() + Duration::from_millis(ms as u64));
        self.hecate.queue_redraw_all();
        let _ = self.hecate.event_loop.insert_source(
            Timer::from_duration(Duration::from_millis(ms as u64 + 10)),
            |_, _, state| {
                state.hecate.queue_redraw_all();
                TimeoutAction::Drop
            },
        );
    }

    /// Colour of the composited pixel at a global position, as `#rrggbb`.
    pub fn pixel_at(&mut self, pos: Point<f64, Logical>) -> Option<String> {
        let output = self.hecate.wm.output_at(pos)?.clone();
        let scale = output.current_scale().fractional_scale();
        let geo = Wm::output_geometry(&output);
        let local = (pos - geo.loc.to_f64()).to_physical(scale).to_i32_round::<i32>();
        let State { backend, hecate } = self;
        backend.with_renderer(|renderer| capture_pixel(renderer, hecate, &output, local))?
    }

    pub fn ipc_gaming(&self) -> ipc::Gaming {
        let outputs = self
            .hecate
            .wm
            .outputs()
            .map(|o| {
                let windows = self.hecate.wm.windows_on_output(o);
                let fullscreen = windows.iter().any(|w| w.data().fullscreen);
                let vrr = self
                    .hecate
                    .config
                    .outputs
                    .get(&o.name())
                    .and_then(|c| c.adaptive_sync)
                    .unwrap_or_default();
                let blocker = if !self.hecate.config.gaming.direct_scanout {
                    Some("gaming.direct_scanout = false".to_string())
                } else if !fullscreen {
                    Some("no fullscreen window".to_string())
                } else if windows.iter().rev().skip(1).any(|w| !w.data().fullscreen) {
                    Some("other windows above the fullscreen one".to_string())
                } else {
                    None
                };
                ipc::GamingOutput {
                    name: o.name(),
                    refresh_hz: o.current_mode().map(|m| m.refresh as f64 / 1000.0).unwrap_or(0.0),
                    vrr_supported: false,
                    vrr_enabled: matches!(vrr, crate::config::Vrr::On)
                        || (matches!(vrr, crate::config::Vrr::Fullscreen) && fullscreen),
                    tearing_allowed: match self.hecate.config.gaming.allow_tearing {
                        crate::config::Tearing::Never => false,
                        crate::config::Tearing::Fullscreen => fullscreen,
                        crate::config::Tearing::Always => true,
                    },
                    direct_scanout: blocker.is_none(),
                    direct_scanout_blocker: blocker,
                }
            })
            .collect();
        let windows = self
            .hecate
            .wm
            .visible_windows()
            .iter()
            .map(|w| {
                let d = w.data();
                ipc::GamingWindow {
                    id: d.id,
                    app_id: None,
                    content_type: None,
                    tearing_requested: d.rules.tearing,
                    pointer_locked: false,
                    pointer_confined: false,
                    fullscreen: d.fullscreen,
                }
            })
            .collect();
        ipc::Gaming { outputs, windows }
    }
}

/// Render the output offscreen and read one pixel.
fn capture_pixel(
    renderer: &mut GlesRenderer,
    hecate: &Hecate,
    output: &smithay::output::Output,
    at: Point<i32, smithay::utils::Physical>,
) -> Option<String> {
    let mode = output.current_mode()?;
    let size: Size<i32, smithay::utils::Buffer> = (mode.size.w, mode.size.h).into();
    let mut tex: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, size).ok()?;
    let elements = hecate.render_elements(renderer, output);
    let mut tracker = OutputDamageTracker::from_output(output);
    let mut fb = renderer.bind(&mut tex).ok()?;
    tracker
        .render_output(renderer, &mut fb, 0, &elements, [0., 0., 0., 1.])
        .ok()?;
    let x = at.x.clamp(0, mode.size.w - 1);
    let y = at.y.clamp(0, mode.size.h - 1);
    let mapping = renderer
        .copy_framebuffer(
            &fb,
            Rectangle::new((x, y).into(), (1, 1).into()),
            Fourcc::Abgr8888,
        )
        .ok()?;
    let bytes = renderer.map_texture(&mapping).ok()?;
    let [r, g, b] = [bytes[0], bytes[1], bytes[2]];
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}
