//! Commit synchronisation: buffer readiness (implicit fences and drm-syncobj acquire points),
//! wp-fifo barriers and wp-commit-timing deadlines.
//!
//! Follows Smithay's anvil (MIT).

use std::collections::HashMap;
use std::time::Duration;

use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::CursorImageStatus;
use smithay::output::Output;
use smithay::reexports::calloop::Interest;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Monotonic, Time};
use smithay::wayland::commit_timing::CommitTimerBarrierStateUserData;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorHandler, SurfaceAttributes, SurfaceData, TraversalAction, add_blocker,
    add_pre_commit_hook, with_states, with_surface_tree_downward,
};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::drm_syncobj::{DrmSyncobjCachedState, DrmSyncobjHandler, DrmSyncobjState};
use smithay::wayland::fifo::FifoBarrierCachedState;

use crate::state::State;

impl DrmSyncobjHandler for State {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        match &mut self.backend {
            crate::backend::Backend::Tty(tty) => tty.syncobj_state.as_mut(),
            _ => None,
        }
    }
}

fn for_each_surface_in_tree(root: &WlSurface, f: &mut impl FnMut(&WlSurface, &SurfaceData)) {
    with_surface_tree_downward(
        root,
        (),
        |_, _, _| TraversalAction::DoChildren(()),
        |surface, states, _| f(surface, states),
        |_, _, _| true,
    );
}

impl State {
    /// Hold back commits whose buffer isn't ready: wait on the drm-syncobj acquire point, or
    /// on the dmabuf's implicit fence.
    pub fn add_commit_blockers(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, |state, _dh, surface| {
            let (acquire, dmabuf) = with_states(surface, |data| {
                let acquire = data
                    .cached_state
                    .get::<DrmSyncobjCachedState>()
                    .pending()
                    .acquire_point
                    .clone();
                let dmabuf = data
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .pending()
                    .buffer
                    .as_ref()
                    .and_then(|b| match b {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).cloned().ok(),
                        _ => None,
                    });
                (acquire, dmabuf)
            });
            let Some(dmabuf) = dmabuf else {
                return;
            };
            let Some(client) = surface.client() else {
                return;
            };
            if let Some(acquire) = acquire
                && let Ok((blocker, source)) = acquire.generate_blocker()
            {
                let c = client.clone();
                let res = state.hecate.event_loop.insert_source(source, move |_, _, state| {
                    state.unblock_client(&c);
                    Ok(())
                });
                if res.is_ok() {
                    add_blocker(surface, blocker);
                    return;
                }
            }
            if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                let res = state.hecate.event_loop.insert_source(source, move |_, _, state| {
                    state.unblock_client(&client);
                    Ok(())
                });
                if res.is_ok() {
                    add_blocker(surface, blocker);
                }
            }
        });
    }

    /// Re-run commits held by blockers that have been released.
    pub fn unblock_client(&mut self, client: &Client) {
        let dh = self.hecate.display_handle.clone();
        self.client_compositor_state(client).blocker_cleared(self, &dh);
    }

    fn unblock_clients(&mut self, clients: HashMap<ClientId, Client>) {
        for client in clients.into_values() {
            self.unblock_client(&client);
        }
    }

    /// Root surfaces shown on an output (all windows when `None`).
    fn surfaces_for(&self, output: Option<&Output>) -> Vec<WlSurface> {
        let mut out: Vec<WlSurface> = match output {
            Some(o) => self.hecate.wm.windows_on_output(o),
            None => self.hecate.wm.all_windows().cloned().collect(),
        }
        .iter()
        .filter_map(|w| w.toplevel().map(|t| t.wl_surface().clone()))
        .collect();
        let outputs: Vec<Output> = match output {
            Some(o) => vec![o.clone()],
            None => self.hecate.wm.outputs().cloned().collect(),
        };
        for o in &outputs {
            let map = layer_map_for_output(o);
            out.extend(map.layers().map(|l| l.wl_surface().clone()));
            drop(map);
            if let Some(lock) = self.hecate.wm.lock_surface(o) {
                out.push(lock.wl_surface().clone());
            }
        }
        if let CursorImageStatus::Surface(s) = &self.hecate.cursor_status {
            out.push(s.clone());
        }
        if let Some(icon) = &self.hecate.dnd_icon {
            out.push(icon.clone());
        }
        // Popups are separate roots.
        for popup in self.hecate.popups_for(&out) {
            out.push(popup);
        }
        out
    }

    /// Release commit-timing barriers due by `deadline`.
    fn signal_commit_timers(&mut self, output: Option<&Output>, deadline: Duration) -> Option<Duration> {
        let target: Time<Monotonic> = deadline.into();
        let mut clients = HashMap::new();
        let mut next: Option<Duration> = None;
        for root in self.surfaces_for(output) {
            for_each_surface_in_tree(&root, &mut |surface, states| {
                if let Some(mut timer) = states
                    .data_map
                    .get::<CommitTimerBarrierStateUserData>()
                    .map(|t| t.lock().unwrap())
                {
                    if timer.signal_until(target)
                        && let Some(c) = surface.client()
                    {
                        clients.insert(c.id(), c);
                    }
                    if let Some(d) = timer.next_deadline() {
                        let d: Duration = Time::<Monotonic>::from(d).into();
                        next = Some(next.map_or(d, |n| n.min(d)));
                    }
                }
            });
        }
        self.unblock_clients(clients);
        next
    }

    /// Release fifo barriers of surfaces on `output` (all surfaces when `None`).
    fn signal_fifo(&mut self, output: Option<&Output>) {
        let mut clients = HashMap::new();
        for root in self.surfaces_for(output) {
            for_each_surface_in_tree(&root, &mut |surface, states| {
                let barrier = states
                    .cached_state
                    .get::<FifoBarrierCachedState>()
                    .current()
                    .barrier
                    .take();
                if let Some(barrier) = barrier {
                    barrier.signal();
                    if let Some(c) = surface.client() {
                        clients.insert(c.id(), c);
                    }
                }
            });
        }
        self.unblock_clients(clients);
    }

    /// Before rendering `output`: let through timed commits meant for this frame.
    pub fn pre_repaint(&mut self, output: &Output) {
        let refresh = output.current_mode().map(|m| m.refresh).unwrap_or(60_000).max(1);
        let frame = Duration::from_secs_f64(1000.0 / refresh as f64);
        self.signal_commit_timers(Some(output), crate::utils::monotonic_time() + frame);
    }

    /// After a frame for `output` was submitted: the fifo barriers it covered are done.
    pub fn post_repaint(&mut self, output: &Output) {
        self.signal_fifo(Some(output));
    }

    /// Release due commit-timing barriers and arm a timer for the next deadline. Runs every loop
    /// iteration so a timed commit lands on time even when nothing redraws.
    pub fn refresh_commit_timers(&mut self) {
        let now = crate::utils::monotonic_time();
        let Some(next) = self.signal_commit_timers(None, now) else {
            return;
        };
        if self.hecate.commit_timer_armed {
            return;
        }
        self.hecate.commit_timer_armed = true;
        let res = self.hecate.event_loop.insert_source(
            Timer::from_duration(next.saturating_sub(now)),
            |_, _, state| {
                state.hecate.commit_timer_armed = false;
                state.refresh_commit_timers();
                TimeoutAction::Drop
            },
        );
        if res.is_err() {
            self.hecate.commit_timer_armed = false;
        }
    }

    /// Surfaces that aren't being drawn (hidden workspaces, off-screen) still make progress:
    /// their fifo barriers are released on the fallback tick.
    pub fn sync_fallback(&mut self) {
        self.signal_fifo(None);
    }
}
