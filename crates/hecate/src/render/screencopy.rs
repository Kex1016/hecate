//! Serving wlr-screencopy frames.
//!
//! Adapted from niri (src/niri.rs), GPL-3.0-or-later.

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};
use smithay::backend::renderer::element::{Element, RenderElement, RenderElementStates};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::output::Output;
use smithay::output::OutputModeSource;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::utils::{Physical, Rectangle, Scale, Transform};

use super::HecateElement;
use super::capture::{render_to_dmabuf, render_to_shm};
use crate::protocols::screencopy::{Screencopy, ScreencopyBuffer};
use crate::state::{Hecate, State};

impl State {
    /// A client asked to copy a frame: render now, or queue until the output has damage.
    pub fn screencopy_frame(&mut self, manager: &ZwlrScreencopyManagerV1, screencopy: Screencopy) {
        if !self.hecate.output_state.contains_key(screencopy.output()) {
            // Dropping fails the frame.
            return;
        }
        if screencopy.with_damage() {
            let output = screencopy.output().clone();
            self.hecate.screencopy_state.push(manager, screencopy);
            self.hecate.queue_redraw(&output);
        } else {
            let State { backend, hecate } = self;
            let res = backend
                .with_renderer(|r| hecate.render_for_screencopy_without_damage(r, manager, screencopy));
            match res {
                Some(Err(err)) => tracing::warn!("error rendering for screencopy: {err:?}"),
                None => tracing::debug!("screencopy without a renderer"),
                Some(Ok(())) => (),
            }
        }
    }
}

impl Hecate {
    fn screencopy_elements(
        &self,
        renderer: &mut GlesRenderer,
        output: &Output,
        screencopy: &Screencopy,
    ) -> Vec<RelocateRenderElement<HecateElement>> {
        let offset = screencopy.region_loc().upscale(-1);
        self.render_elements_for(renderer, output, screencopy.overlay_cursor())
            .into_iter()
            .map(|e| RelocateRenderElement::from_element(e, offset, Relocate::Relative))
            .collect()
    }

    pub fn render_for_screencopy_with_damage(&mut self, renderer: &mut GlesRenderer, output: &Output) {
        let mut state = std::mem::take(&mut self.screencopy_state);
        state.with_queues_mut(|queue| {
            let (damage_tracker, Some(screencopy)) = queue.split() else {
                return;
            };
            if screencopy.output() != output {
                return;
            }
            let elements = self.screencopy_elements(renderer, output, screencopy);
            let (damages, states) = damage_screencopy(output, &elements, damage_tracker, screencopy);
            let Some(damages) = damages else {
                // Nothing changed; wait for the next redraw.
                return;
            };
            // Physical back to buffer coordinates.
            let transform = output.current_transform();
            let physical_size = transform.transform_size(screencopy.buffer_size());
            screencopy.damage(damages.iter().map(|d| {
                d.to_logical(1)
                    .to_buffer(1, transform.invert(), &physical_size.to_logical(1))
            }));
            match render_screencopy(renderer, damage_tracker, &elements, states, screencopy) {
                Ok(sync) => queue.pop().submit_after_sync(sync, &self.event_loop),
                Err(err) => {
                    queue.reset_damage();
                    queue.pop();
                    tracing::warn!("error rendering for screencopy: {err:?}");
                }
            }
        });
        self.screencopy_state = state;
    }

    fn render_for_screencopy_without_damage(
        &mut self,
        renderer: &mut GlesRenderer,
        manager: &ZwlrScreencopyManagerV1,
        screencopy: Screencopy,
    ) -> anyhow::Result<()> {
        let output = screencopy.output().clone();
        let elements = self.screencopy_elements(renderer, &output, &screencopy);
        let Some(damage_tracker) = self.screencopy_state.damage_tracker(manager) else {
            anyhow::bail!("screencopy queue missing");
        };
        let (_, states) = damage_screencopy(&output, &elements, damage_tracker, &screencopy);
        let res = render_screencopy(renderer, damage_tracker, &elements, states, &screencopy);
        match res {
            Ok(sync) => {
                screencopy.submit_after_sync(sync, &self.event_loop);
                Ok(())
            }
            Err(err) => {
                *damage_tracker = OutputDamageTracker::new((0, 0), 1.0, Transform::Normal);
                Err(err)
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn damage_screencopy<'a>(
    output: &Output,
    elements: &[impl Element],
    damage_tracker: &'a mut OutputDamageTracker,
    screencopy: &Screencopy,
) -> (Option<&'a Vec<Rectangle<i32, Physical>>>, RenderElementStates) {
    let OutputModeSource::Static {
        size: last_size,
        scale: last_scale,
        transform: last_transform,
    } = damage_tracker.mode().clone()
    else {
        unreachable!("screencopy damage trackers have a static mode");
    };
    let size = screencopy.buffer_size();
    let scale: Scale<f64> = output.current_scale().fractional_scale().into();
    let transform = output.current_transform();
    if size != last_size || scale != last_scale || transform != last_transform {
        *damage_tracker = OutputDamageTracker::new(size, scale, transform);
    }
    damage_tracker.damage_output(1, elements).unwrap()
}

fn render_screencopy(
    renderer: &mut GlesRenderer,
    damage_tracker: &mut OutputDamageTracker,
    elements: &[impl RenderElement<GlesRenderer>],
    states: RenderElementStates,
    screencopy: &Screencopy,
) -> anyhow::Result<Option<SyncPoint>> {
    Ok(match screencopy.buffer() {
        ScreencopyBuffer::Dmabuf(dmabuf) => Some(render_to_dmabuf(
            renderer,
            damage_tracker,
            dmabuf.clone(),
            elements,
            states,
        )?),
        ScreencopyBuffer::Shm(buffer) => {
            render_to_shm(
                renderer,
                damage_tracker,
                buffer,
                wl_shm::Format::Xrgb8888,
                elements,
                states,
            )?;
            None
        }
    })
}
