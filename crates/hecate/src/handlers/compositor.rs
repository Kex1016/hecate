use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::PopupKind;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes, get_parent,
    is_sync_subsurface, with_states,
};

use crate::state::{ClientState, State};
use crate::wm::WindowExt;

impl BufferHandler for State {
    fn buffer_destroyed(
        &mut self,
        _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    ) {
    }
}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.hecate.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        self.add_commit_blockers(surface);
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.backend.early_import(surface);

        if is_sync_subsurface(surface) {
            return;
        }
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }

        if &root == surface {
            // An unmapped toplevel: send the initial configure, map once it has a buffer.
            if let Some(idx) = self
                .hecate
                .wm
                .pending
                .iter()
                .position(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            {
                let window = self.hecate.wm.pending[idx].clone();
                let toplevel = window.toplevel_surface().clone();
                if !toplevel.is_initial_configure_sent() {
                    self.apply_window_rules(&window, true);
                    self.prepare_initial_configure(&window);
                    toplevel.send_configure();
                } else if crate::utils::is_mapped(surface) {
                    self.hecate.wm.pending.remove(idx);
                    window.on_commit();
                    self.map_new_window(window);
                }
                return;
            }

            if let Some(window) = self.hecate.window_for_surface(surface) {
                if !crate::utils::is_mapped(surface) {
                    // The client unmapped the window (null buffer): back to pending.
                    let output = self.hecate.wm.output_for_window(&window).cloned();
                    self.hecate.wm.remove_window(&window);
                    self.on_window_closed(&window);
                    window.toplevel_surface().with_pending_state(|s| s.size = None);
                    self.hecate.wm.pending.push(window);
                    if let Some(o) = output {
                        self.hecate.queue_redraw(&o);
                    }
                    self.focus_topmost();
                    return;
                }
                window.on_commit();
                self.on_window_commit(&window);
                if let Some(o) = self.hecate.wm.output_for_window(&window).cloned() {
                    self.hecate.queue_redraw(&o);
                }
                return;
            }
        }

        // Popups.
        self.hecate.popups.commit(surface);
        if let Some(popup) = self.hecate.popups.find_popup(surface) {
            if let PopupKind::Xdg(ref xdg) = popup
                && !xdg.is_initial_configure_sent()
                && let Err(err) = xdg.send_configure()
            {
                tracing::warn!("error sending initial popup configure: {err}");
            }
            self.hecate.queue_redraw_all();
            return;
        }

        // Layer surfaces.
        if self.layer_surface_commit(&root) {
            return;
        }

        // Cursor surface: track the hotspot.
        if matches!(&self.hecate.cursor_status, CursorImageStatus::Surface(s) if s == surface) {
            with_states(surface, |states| {
                if let Some(mut attrs) = states
                    .data_map
                    .get::<CursorImageSurfaceData>()
                    .map(|a| a.lock().unwrap())
                {
                    let delta = states
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .current()
                        .buffer_delta
                        .take();
                    if let Some(delta) = delta {
                        attrs.hotspot -= delta;
                    }
                }
            });
        }

        // Lock surfaces, subsurfaces of anything, cursor, DnD icon: just redraw.
        self.hecate.queue_redraw_all();
    }
}
