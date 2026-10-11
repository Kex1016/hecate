//! ext-image-copy-capture for outputs and toplevels (QuickShell's ScreencopyView, grim, portals).
//!
//! Adapted from niri (src/handlers/image_copy_capture.rs), GPL-3.0-or-later.

use std::time::Duration;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer as _, Fourcc, Modifier};
use smithay::backend::drm::DrmNode;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::desktop::Window;
use smithay::output::{Output, OutputModeSource, WeakOutput};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::server::ext_foreign_toplevel_image_capture_source_manager_v1::{
    self, ExtForeignToplevelImageCaptureSourceManagerV1,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New};
use smithay::utils::{Buffer as BufferCoords, Physical, Point, Rectangle, Scale, Size, Transform};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::image_capture_source::{
    ImageCaptureSource, ImageCaptureSourceData, ImageCaptureSourceHandler, OutputCaptureSourceHandler,
    OutputCaptureSourceState,
};
use smithay::wayland::image_copy_capture::{
    BufferConstraints, CaptureFailureReason, DmabufConstraints, Frame, FrameRef, ImageCopyCaptureHandler,
    ImageCopyCaptureState, Session, SessionRef,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2, shm};

use crate::protocols::EmptyData;
use crate::render::HecateElement;
use crate::render::capture::{render_to_dmabuf, render_to_shm};
use crate::state::State;

/// What a capture source points at; stored in the source's user data.
#[derive(Debug, Clone)]
pub enum CaptureTarget {
    Output(WeakOutput),
    Window(u64),
}

pub struct ImageCopySession {
    pub session: Session,
    pub damage_tracker: OutputDamageTracker,
    /// Frame waiting for damage.
    pub pending_frame: Option<Frame>,
}

fn target(source: &ImageCaptureSource) -> Option<CaptureTarget> {
    source.user_data().get::<CaptureTarget>().cloned()
}

/// A resolved capture target: what to render and how big.
enum Live {
    Output(Output),
    Window(Window, Scale<f64>),
}

impl Live {
    /// Buffer size, scale and transform the damage tracker works in.
    fn mode(&self) -> Option<(Size<i32, Physical>, Scale<f64>, Transform)> {
        match self {
            Live::Output(o) => Some((
                o.current_mode()?.size,
                o.current_scale().fractional_scale().into(),
                o.current_transform(),
            )),
            Live::Window(w, scale) => {
                let size = w.geometry().size.to_physical_precise_round(*scale);
                (!size.is_empty()).then_some((size, *scale, Transform::Normal))
            }
        }
    }
}

fn resolve(state: &State, source: &ImageCaptureSource) -> Option<Live> {
    match target(source)? {
        CaptureTarget::Output(weak) => {
            let o = weak.upgrade()?;
            state
                .hecate
                .output_state
                .contains_key(&o)
                .then_some(Live::Output(o))
        }
        CaptureTarget::Window(id) => {
            // Mapped windows only (unmapped toplevels wait in `pending`).
            let w = state.hecate.wm.window_by_id(id)?;
            state.hecate.wm.workspace_of(&w)?;
            let scale = state
                .hecate
                .wm
                .output_for_window(&w)
                .map(|o| o.current_scale().fractional_scale())
                .unwrap_or(1.0);
            Some(Live::Window(w, scale.into()))
        }
    }
}

fn constraints(
    renderer: &GlesRenderer,
    node: Option<DrmNode>,
    size: Size<i32, Physical>,
) -> BufferConstraints {
    let dma = node.and_then(|node| {
        let mut formats: Vec<(Fourcc, Vec<Modifier>)> = Vec::new();
        for format in renderer.egl_context().dmabuf_render_formats().iter() {
            match formats.iter_mut().find(|(code, _)| *code == format.code) {
                Some((_, modifiers)) => modifiers.push(format.modifier),
                None => formats.push((format.code, vec![format.modifier])),
            }
        }
        // Some clients take the first format (wl-mirror, grim).
        formats.sort_by_key(|(code, _)| match code {
            Fourcc::Xrgb8888 => 0,
            Fourcc::Argb8888 => 1,
            _ => 2,
        });
        (!formats.is_empty()).then_some(DmabufConstraints { node, formats })
    });
    BufferConstraints {
        size: (size.w, size.h).into(),
        shm: vec![wl_shm::Format::Xrgb8888],
        dma,
    }
}

enum CaptureBuffer {
    Dma(Dmabuf),
    Shm,
}

/// Smithay allows larger buffers than asked for; the render helpers need an exact match.
fn capture_buffer(buffer: &WlBuffer, size: Size<i32, BufferCoords>) -> Option<CaptureBuffer> {
    if let Ok(dmabuf) = get_dmabuf(buffer) {
        let ok = dmabuf.width() == size.w as u32 && dmabuf.height() == size.h as u32;
        return ok.then(|| CaptureBuffer::Dma(dmabuf.clone()));
    }
    shm::with_buffer_contents(buffer, |_, _, data| {
        data.format == wl_shm::Format::Xrgb8888 && data.width == size.w && data.height == size.h
    })
    .unwrap_or(false)
    .then_some(CaptureBuffer::Shm)
}

fn success_after_sync(
    frame: Frame,
    transform: Transform,
    damage: Vec<Rectangle<i32, BufferCoords>>,
    presented: Duration,
    sync: Option<SyncPoint>,
    event_loop: &LoopHandle<'static, State>,
) {
    let Some(fd) = sync.and_then(|s| s.export()) else {
        frame.success(transform, damage, presented);
        return;
    };
    let mut pending = Some((frame, damage));
    let res = event_loop.insert_source(Generic::new(fd, Interest::READ, Mode::OneShot), move |_, _, _| {
        if let Some((frame, damage)) = pending.take() {
            frame.success(transform, damage, presented);
        }
        Ok(PostAction::Remove)
    });
    if let Err(err) = res {
        tracing::warn!("error waiting for image copy sync: {err}");
    }
}

impl State {
    /// Drop sessions whose source is gone, renegotiate on size changes, and complete frames
    /// that have damage. Runs once per loop iteration.
    pub fn refresh_image_copy_capture(&mut self) {
        if self.hecate.image_copy_sessions.is_empty() {
            return;
        }
        let mut sessions = std::mem::take(&mut self.hecate.image_copy_sessions);
        let node = self.backend.render_node();

        // Dropping a session sends `stopped`.
        sessions.retain_mut(|s| {
            let Some(live) = resolve(self, &s.session.source()) else {
                return false;
            };
            let Some((size, _, _)) = live.mode() else {
                return false;
            };
            if s.session.current_constraints().map(|c| c.size) == Some((size.w, size.h).into()) {
                return true;
            }
            let Some(c) = self.backend.with_renderer(|r| constraints(r, node, size)) else {
                return false;
            };
            // Frames can't be captured against outdated constraints.
            if let Some(frame) = s.pending_frame.take() {
                frame.fail(CaptureFailureReason::BufferConstraints);
            }
            s.session.update_constraints(c);
            true
        });

        for s in sessions.iter_mut().filter(|s| s.pending_frame.is_some()) {
            let Some(live) = resolve(self, &s.session.source()) else {
                continue;
            };
            let draw_cursor = s.session.draw_cursor();
            let State { backend, hecate } = &mut *self;
            backend.with_renderer(|renderer| {
                let Some((size, scale, transform)) = live.mode() else {
                    return;
                };
                let OutputModeSource::Static {
                    size: last_size,
                    scale: last_scale,
                    transform: last_transform,
                } = s.damage_tracker.mode().clone()
                else {
                    unreachable!("capture damage trackers have a static mode");
                };
                if size != last_size || scale != last_scale || transform != last_transform {
                    s.damage_tracker = OutputDamageTracker::new(size, scale, transform);
                }

                let elements: Vec<HecateElement> = match &live {
                    Live::Output(o) => hecate.render_elements_for(renderer, o, draw_cursor),
                    Live::Window(w, scale) => {
                        let loc: Point<i32, Physical> =
                            w.geometry().loc.upscale(-1).to_physical_precise_round(*scale);
                        AsRenderElements::<GlesRenderer>::render_elements(w, renderer, loc, *scale, 1.0)
                    }
                };
                let (damage, states) = s.damage_tracker.damage_output(1, &elements).unwrap();
                let Some(damage) = damage else {
                    return;
                };
                let physical_size = transform.transform_size(size);
                let damage: Vec<Rectangle<i32, BufferCoords>> = damage
                    .iter()
                    .map(|d| {
                        d.to_logical(1)
                            .to_buffer(1, transform.invert(), &physical_size.to_logical(1))
                    })
                    .collect();

                let frame = s.pending_frame.take().unwrap();
                let buffer = frame.buffer();
                let Some(target) = capture_buffer(&buffer, (size.w, size.h).into()) else {
                    frame.fail(CaptureFailureReason::BufferConstraints);
                    s.damage_tracker = OutputDamageTracker::new(size, scale, transform);
                    return;
                };
                let res = match target {
                    CaptureBuffer::Dma(dmabuf) => {
                        render_to_dmabuf(renderer, &mut s.damage_tracker, dmabuf, &elements, states).map(Some)
                    }
                    CaptureBuffer::Shm => render_to_shm(
                        renderer,
                        &mut s.damage_tracker,
                        &buffer,
                        wl_shm::Format::Xrgb8888,
                        &elements,
                        states,
                    )
                    .map(|()| None),
                };
                match res {
                    Ok(sync) => success_after_sync(
                        frame,
                        transform,
                        damage,
                        crate::utils::monotonic_time(),
                        sync,
                        &hecate.event_loop,
                    ),
                    Err(err) => {
                        tracing::warn!("error rendering for image copy: {err:?}");
                        frame.fail(CaptureFailureReason::Unknown);
                        s.damage_tracker = OutputDamageTracker::new(size, scale, transform);
                    }
                }
            });
        }

        sessions.append(&mut self.hecate.image_copy_sessions);
        self.hecate.image_copy_sessions = sessions;
    }
}

impl ImageCaptureSourceHandler for State {}

impl OutputCaptureSourceHandler for State {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.hecate.output_capture_source_state
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &Output) {
        source
            .user_data()
            .insert_if_missing(|| CaptureTarget::Output(output.downgrade()));
    }
}

impl ImageCopyCaptureHandler for State {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.hecate.image_copy_capture_state
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let live = resolve(self, source)?;
        let (size, _, _) = live.mode()?;
        let node = self.backend.render_node();
        self.backend.with_renderer(|r| constraints(r, node, size))
    }

    fn new_session(&mut self, session: Session) {
        self.hecate.image_copy_sessions.push(ImageCopySession {
            session,
            damage_tracker: OutputDamageTracker::new((0, 0), 1.0, Transform::Normal),
            pending_frame: None,
        });
    }

    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        let Some(s) = self
            .hecate
            .image_copy_sessions
            .iter_mut()
            .find(|s| s.session == *session)
        else {
            frame.fail(CaptureFailureReason::Unknown);
            return;
        };
        // One frame in flight per session (a client bug otherwise).
        if s.pending_frame.is_some() {
            frame.fail(CaptureFailureReason::Unknown);
            return;
        }
        s.pending_frame = Some(frame);
    }

    fn frame_aborted(&mut self, frame: FrameRef) {
        for s in &mut self.hecate.image_copy_sessions {
            if s.pending_frame.as_deref() == Some(&frame) {
                s.pending_frame = None;
            }
        }
    }

    fn session_destroyed(&mut self, session: SessionRef) {
        let sessions = &mut self.hecate.image_copy_sessions;
        if let Some(idx) = sessions.iter().position(|s| s.session == session) {
            let s = sessions.remove(idx);
            if let Some(frame) = s.pending_frame {
                frame.fail(CaptureFailureReason::Stopped);
            }
        }
        self.hecate.image_copy_capture_state.cleanup();
    }
}

// ext-foreign-toplevel-image-capture-source: our foreign-toplevel list is our own, so Smithay's
// manager (which expects its own handles) doesn't fit.

pub struct ToplevelCaptureSourceGlobalData {
    pub filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl GlobalDispatch2<ExtForeignToplevelImageCaptureSourceManagerV1, State>
    for ToplevelCaptureSourceGlobalData
{
    fn bind(
        &self,
        _state: &mut State,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ExtForeignToplevelImageCaptureSourceManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, EmptyData);
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ExtForeignToplevelImageCaptureSourceManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        _resource: &ExtForeignToplevelImageCaptureSourceManagerV1,
        request: ext_foreign_toplevel_image_capture_source_manager_v1::Request,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            ext_foreign_toplevel_image_capture_source_manager_v1::Request::CreateSource {
                source,
                toplevel_handle,
            } => {
                let capture_source = ImageCaptureSource::new();
                let resource = data_init.init(
                    source,
                    ImageCaptureSourceData {
                        source: capture_source.clone(),
                    },
                );
                capture_source.add_instance(&resource);
                // An unknown or closed handle gives a source that can't be captured.
                if let Some(id) = state
                    .hecate
                    .foreign_toplevel_state
                    .window_id_for_ext_handle(&toplevel_handle)
                {
                    capture_source
                        .user_data()
                        .insert_if_missing(|| CaptureTarget::Window(id));
                }
            }
            ext_foreign_toplevel_image_capture_source_manager_v1::Request::Destroy => (),
            _ => unreachable!(),
        }
    }
}
