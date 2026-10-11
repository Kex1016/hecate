//! wlr-screencopy-unstable-v1 (grim, wf-recorder, xdg-desktop-portal-wlr, QuickShell).
//!
//! Adapted from niri (src/protocols/screencopy.rs), GPL-3.0-or-later.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer, Fourcc};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::{
    self, Flags, ZwlrScreencopyFrameV1,
};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::{
    self, ZwlrScreencopyManagerV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm::Format;
use smithay::reexports::wayland_server::{Client, DataInit, DisplayHandle, New, Resource};
use smithay::utils::{Physical, Point, Rectangle, Size, Transform};
use smithay::wayland::{Dispatch2, GlobalDispatch2, dmabuf, shm};

use super::EmptyData;
use crate::state::State;

const VERSION: u32 = 3;

fn shm_stride(size: Size<i32, Physical>) -> i32 {
    size.w * 4
}

pub struct ScreencopyQueue {
    damage_tracker: OutputDamageTracker,
    /// Frames waiting for the client to call copy or destroy.
    pending_frames: HashSet<ZwlrScreencopyFrameV1>,
    /// Copies waiting for an output redraw with damage.
    screencopies: Vec<Screencopy>,
}

impl ScreencopyQueue {
    fn new() -> Self {
        Self {
            damage_tracker: OutputDamageTracker::new((0, 0), 1.0, Transform::Normal),
            pending_frames: HashSet::new(),
            screencopies: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending_frames.is_empty() && self.screencopies.is_empty()
    }

    pub fn split(&mut self) -> (&mut OutputDamageTracker, Option<&Screencopy>) {
        (&mut self.damage_tracker, self.screencopies.first())
    }

    pub fn pop(&mut self) -> Screencopy {
        self.screencopies.remove(0)
    }

    /// Forget damage history so the next copy is complete.
    pub fn reset_damage(&mut self) {
        self.damage_tracker = OutputDamageTracker::new((0, 0), 1.0, Transform::Normal);
    }
}

#[derive(Default)]
pub struct ScreencopyManagerState {
    queues: HashMap<ZwlrScreencopyManagerV1, ScreencopyQueue>,
}

pub struct ScreencopyManagerGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

impl ScreencopyManagerState {
    pub fn new<F>(display: &DisplayHandle, filter: F) -> Self
    where
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        display.create_global::<State, ZwlrScreencopyManagerV1, _>(
            VERSION,
            ScreencopyManagerGlobalData {
                filter: Box::new(filter),
            },
        );
        Self::default()
    }

    pub fn push(&mut self, manager: &ZwlrScreencopyManagerV1, screencopy: Screencopy) {
        if let Some(queue) = self.queues.get_mut(manager) {
            queue.screencopies.push(screencopy);
        }
    }

    pub fn damage_tracker(&mut self, manager: &ZwlrScreencopyManagerV1) -> Option<&mut OutputDamageTracker> {
        Some(&mut self.queues.get_mut(manager)?.damage_tracker)
    }

    pub fn remove_output(&mut self, output: &Output) {
        for queue in self.queues.values_mut() {
            queue.screencopies.retain(|s| s.output() != output);
        }
        self.cleanup_queues();
    }

    /// Whether any damage-tracked copy waits on `output`.
    pub fn has_pending_for(&self, output: &Output) -> bool {
        self.queues
            .values()
            .any(|q| q.screencopies.first().is_some_and(|s| s.output() == output))
    }

    pub fn with_queues_mut(&mut self, mut f: impl FnMut(&mut ScreencopyQueue)) {
        for queue in self.queues.values_mut() {
            f(queue);
        }
        self.cleanup_queues();
    }

    fn cleanup_queues(&mut self) {
        self.queues
            .retain(|manager, queue| manager.is_alive() || !queue.is_empty());
    }
}

impl GlobalDispatch2<ZwlrScreencopyManagerV1, State> for ScreencopyManagerGlobalData {
    fn bind(
        &self,
        state: &mut State,
        _dh: &DisplayHandle,
        _client: &Client,
        manager: New<ZwlrScreencopyManagerV1>,
        data_init: &mut DataInit<'_, State>,
    ) {
        let manager = data_init.init(manager, EmptyData);
        state
            .hecate
            .screencopy_state
            .queues
            .insert(manager, ScreencopyQueue::new());
    }

    fn can_view(&self, client: &Client) -> bool {
        (self.filter)(client)
    }
}

impl Dispatch2<ZwlrScreencopyManagerV1, State> for EmptyData {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        manager: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        let (frame, overlay_cursor, buffer_size, region_loc, output) = match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput {
                frame,
                overlay_cursor,
                output,
            } => {
                let Some((output, mode)) = Output::from_resource(&output)
                    .and_then(|o| Some((o.clone(), o.current_mode()?)))
                    .filter(|(o, _)| state.hecate.output_state.contains_key(o))
                else {
                    data_init.init(frame, ScreencopyFrameState::Failed).failed();
                    return;
                };
                (frame, overlay_cursor, mode.size, Point::from((0, 0)), output)
            }
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame,
                overlay_cursor,
                x,
                y,
                width,
                height,
                output,
            } => {
                let found = Output::from_resource(&output)
                    .and_then(|o| Some((o.clone(), o.current_mode()?)))
                    .filter(|(o, _)| state.hecate.output_state.contains_key(o));
                let Some((output, mode)) = found.filter(|_| width > 0 && height > 0) else {
                    data_init.init(frame, ScreencopyFrameState::Failed).failed();
                    return;
                };
                let transform = output.current_transform();
                let physical_size = transform.transform_size(mode.size);
                let rect = Rectangle::new(Point::from((x, y)), Size::from((width, height)));
                let scale = output.current_scale().fractional_scale();
                let physical_rect = rect.to_physical_precise_round(scale);
                // Clamp the region to the output.
                let Some(clamped) = physical_rect.intersection(Rectangle::from_size(physical_size)) else {
                    data_init.init(frame, ScreencopyFrameState::Failed).failed();
                    return;
                };
                let untransformed = transform.invert().transform_rect_in(clamped, &physical_size);
                (frame, overlay_cursor, untransformed.size, clamped.loc, output)
            }
            zwlr_screencopy_manager_v1::Request::Destroy => return,
            _ => unreachable!(),
        };

        let info = ScreencopyFrameInfo {
            output,
            overlay_cursor: overlay_cursor != 0,
            buffer_size,
            region_loc,
        };
        let frame = data_init.init(
            frame,
            ScreencopyFrameState::Pending {
                manager: manager.clone(),
                info,
                copied: Arc::new(AtomicBool::new(false)),
            },
        );

        frame.buffer(
            Format::Xrgb8888,
            buffer_size.w as u32,
            buffer_size.h as u32,
            shm_stride(buffer_size) as u32,
        );
        if frame.version() >= 3 {
            frame.linux_dmabuf(
                Fourcc::Xrgb8888 as u32,
                buffer_size.w as u32,
                buffer_size.h as u32,
            );
            frame.buffer_done();
        }

        if let Some(queue) = state.hecate.screencopy_state.queues.get_mut(manager) {
            queue.pending_frames.insert(frame);
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, manager: &ZwlrScreencopyManagerV1) {
        let queues = &mut state.hecate.screencopy_state.queues;
        if queues.get(manager).is_some_and(ScreencopyQueue::is_empty) {
            queues.remove(manager);
        }
    }
}

#[derive(Clone)]
pub struct ScreencopyFrameInfo {
    output: Output,
    buffer_size: Size<i32, Physical>,
    region_loc: Point<i32, Physical>,
    overlay_cursor: bool,
}

pub enum ScreencopyFrameState {
    Failed,
    Pending {
        manager: ZwlrScreencopyManagerV1,
        info: ScreencopyFrameInfo,
        copied: Arc<AtomicBool>,
    },
}

impl Dispatch2<ZwlrScreencopyFrameV1, State> for ScreencopyFrameState {
    fn request(
        &self,
        state: &mut State,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        if matches!(request, zwlr_screencopy_frame_v1::Request::Destroy) {
            return;
        }
        let ScreencopyFrameState::Pending {
            manager,
            info,
            copied,
        } = self
        else {
            return;
        };
        if copied.load(Ordering::SeqCst) {
            frame.post_error(
                zwlr_screencopy_frame_v1::Error::AlreadyUsed,
                "copy was already requested",
            );
            return;
        }
        let (buffer, with_damage) = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
            _ => unreachable!(),
        };

        let size = info.buffer_size;
        let buffer = if let Ok(dmabuf) = dmabuf::get_dmabuf(&buffer) {
            if dmabuf.format().code == Fourcc::Xrgb8888
                && dmabuf.width() == size.w as u32
                && dmabuf.height() == size.h as u32
            {
                ScreencopyBuffer::Dmabuf(dmabuf.clone())
            } else {
                frame.post_error(
                    zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                    "invalid dmabuf parameters",
                );
                return;
            }
        } else if shm::with_buffer_contents(&buffer, |_, _, data| {
            data.format == Format::Xrgb8888
                && data.width == size.w
                && data.height == size.h
                && data.stride == shm_stride(size)
        })
        .unwrap_or(false)
        {
            ScreencopyBuffer::Shm(buffer)
        } else {
            frame.post_error(zwlr_screencopy_frame_v1::Error::InvalidBuffer, "invalid buffer");
            return;
        };
        copied.store(true, Ordering::SeqCst);

        let screencopy = Screencopy {
            buffer,
            frame: frame.clone(),
            info: info.clone(),
            with_damage,
            submitted: false,
        };
        state.screencopy_frame(manager, screencopy);

        // The frame was copied, failed, or queued by now.
        let ss = &mut state.hecate.screencopy_state;
        if let Some(queue) = ss.queues.get_mut(manager) {
            queue.pending_frames.remove(frame);
            if queue.is_empty() && !manager.is_alive() {
                ss.queues.remove(manager);
            }
        }
    }

    fn destroyed(&self, state: &mut State, _client: ClientId, frame: &ZwlrScreencopyFrameV1) {
        let ScreencopyFrameState::Pending { manager, .. } = self else {
            return;
        };
        let ss = &mut state.hecate.screencopy_state;
        let Some(queue) = ss.queues.get_mut(manager) else {
            return;
        };
        queue.pending_frames.remove(frame);
        queue.screencopies.retain(|s| s.frame != *frame);
        if queue.is_empty() && !manager.is_alive() {
            ss.queues.remove(manager);
        }
    }
}

#[derive(Clone)]
pub enum ScreencopyBuffer {
    Dmabuf(Dmabuf),
    Shm(WlBuffer),
}

pub struct Screencopy {
    info: ScreencopyFrameInfo,
    frame: ZwlrScreencopyFrameV1,
    buffer: ScreencopyBuffer,
    with_damage: bool,
    submitted: bool,
}

impl Drop for Screencopy {
    fn drop(&mut self) {
        if !self.submitted {
            self.frame.failed();
        }
    }
}

impl Screencopy {
    pub fn buffer(&self) -> &ScreencopyBuffer {
        &self.buffer
    }

    pub fn region_loc(&self) -> Point<i32, Physical> {
        self.info.region_loc
    }

    pub fn buffer_size(&self) -> Size<i32, Physical> {
        self.info.buffer_size
    }

    pub fn output(&self) -> &Output {
        &self.info.output
    }

    pub fn overlay_cursor(&self) -> bool {
        self.info.overlay_cursor
    }

    pub fn with_damage(&self) -> bool {
        self.with_damage
    }

    pub fn damage(&self, damages: impl Iterator<Item = Rectangle<i32, smithay::utils::Buffer>>) {
        for Rectangle { loc, size } in damages {
            self.frame
                .damage(loc.x as u32, loc.y as u32, size.w as u32, size.h as u32);
        }
    }

    fn submit(mut self, timestamp: Duration) {
        self.frame.flags(Flags::empty());
        let secs = timestamp.as_secs();
        self.frame.ready(
            (secs >> 32) as u32,
            (secs & 0xFFFF_FFFF) as u32,
            timestamp.subsec_nanos(),
        );
        self.submitted = true;
    }

    /// Send `ready` once rendering finished.
    pub fn submit_after_sync<T>(self, sync_point: Option<SyncPoint>, event_loop: &LoopHandle<'_, T>) {
        let timestamp = crate::utils::monotonic_time();
        match sync_point.and_then(|s| s.export()) {
            None => self.submit(timestamp),
            Some(sync_fd) => {
                let source = Generic::new(sync_fd, Interest::READ, Mode::OneShot);
                let mut screencopy = Some(self);
                let res = event_loop.insert_source(source, move |_, _, _| {
                    if let Some(s) = screencopy.take() {
                        s.submit(timestamp);
                    }
                    Ok(PostAction::Remove)
                });
                if let Err(err) = res {
                    tracing::warn!("error waiting for screencopy sync: {err:?}");
                }
            }
        }
    }
}
