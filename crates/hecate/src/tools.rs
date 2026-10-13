//! Support for the CLI's window tools: identify, colour picking, gaming status.

use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ExportMem, Offscreen};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::state::{Hecate, State};
use crate::wm::Wm;

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
