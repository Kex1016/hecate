//! Running nested inside another compositor (development).

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{ImportDma, ImportEgl};
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::winit::dpi::LogicalSize;
use smithay::reexports::winit::window::WindowAttributes;
use smithay::wayland::dmabuf::DmabufFeedbackBuilder;

use crate::state::{Hecate, State};

pub struct Winit {
    output: Output,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
}

impl Winit {
    pub fn new(event_loop: LoopHandle<'static, State>) -> anyhow::Result<Self> {
        let attrs = WindowAttributes::default()
            .with_surface_size(LogicalSize::new(1280.0, 800.0))
            .with_title("hecate");
        let (backend, winit) =
            winit::init_from_attributes(attrs).map_err(|e| anyhow::anyhow!("winit init: {e}"))?;

        let output = Output::new(
            "winit".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "hecate".into(),
                model: "winit".into(),
                serial_number: "0".into(),
            },
        );
        let mode = Mode {
            size: backend.window_size(),
            refresh: 60_000,
        };
        output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
        output.set_preferred(mode);
        let damage_tracker = OutputDamageTracker::from_output(&output);

        event_loop
            .insert_source(winit, move |event, _, state| match event {
                WinitEvent::Resized { size, .. } => {
                    let output = match &state.backend {
                        crate::backend::Backend::Winit(w) => w.output.clone(),
                        _ => return,
                    };
                    output.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        None,
                        None,
                        None,
                    );
                    state.hecate.update_output_buffers(&output);
                    smithay::desktop::layer_map_for_output(&output).arrange();
                    state.on_usable_area_changed(&output);
                    state.hecate.queue_redraw(&output);
                }
                WinitEvent::Input(event) => state.process_input_event(event),
                WinitEvent::Focus(_) => {}
                WinitEvent::Redraw => state.hecate.queue_redraw_all(),
                WinitEvent::CloseRequested => state.hecate.stop_signal.stop(),
            })
            .map_err(|e| anyhow::anyhow!("inserting winit source: {e}"))?;

        Ok(Winit {
            output,
            backend,
            damage_tracker,
        })
    }

    pub fn init(&mut self, hecate: &mut Hecate) {
        let renderer = self.backend.renderer();
        if renderer.bind_wl_display(&hecate.display_handle).is_ok() {
            tracing::debug!("bound EGL to wl_display");
        }
        crate::render::border::Shaders::init(renderer);

        let formats = renderer.dmabuf_formats();
        let node = smithay::backend::egl::EGLDevice::device_for_display(renderer.egl_context().display())
            .ok()
            .and_then(|d| d.try_get_render_node().ok().flatten());
        let global = match node.and_then(|n| {
            DmabufFeedbackBuilder::new(n.dev_id(), formats.clone())
                .build()
                .ok()
        }) {
            Some(feedback) => hecate
                .dmabuf_state
                .create_global_with_default_feedback::<State>(&hecate.display_handle, &feedback),
            None => hecate
                .dmabuf_state
                .create_global::<State>(&hecate.display_handle, formats),
        };
        let _ = global;

        hecate.add_output(self.output.clone());
    }

    pub fn render(&mut self, hecate: &mut Hecate, output: &Output) -> bool {
        let elements = hecate.render_elements(self.backend.renderer(), output);
        let res = {
            let Ok((renderer, mut fb)) = self.backend.bind() else {
                return false;
            };
            match self
                .damage_tracker
                .render_output(renderer, &mut fb, 0, &elements, [0., 0., 0., 1.])
            {
                Ok(r) => r,
                Err(err) => {
                    tracing::warn!("render error: {err:?}");
                    return false;
                }
            }
        };
        let submitted = if let Some(damage) = res.damage {
            if let Err(err) = self.backend.submit(Some(damage)) {
                tracing::warn!("submit error: {err:?}");
            }
            true
        } else {
            false
        };
        if hecate
            .output_state
            .get(output)
            .is_some_and(|s| s.animations_running)
        {
            self.backend.window().request_redraw();
        }
        submitted
    }

    pub fn renderer(&mut self) -> &mut GlesRenderer {
        self.backend.renderer()
    }

    pub fn import_dmabuf(&mut self, dmabuf: &Dmabuf) -> bool {
        self.backend.renderer().import_dmabuf(dmabuf, None).is_ok()
    }
}
