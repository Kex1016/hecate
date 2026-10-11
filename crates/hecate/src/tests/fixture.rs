//! Test fixture: a headless hecate plus test clients on one event loop.
//!
//! Structure adapted from niri's test fixture (https://github.com/YaLTeR/niri, GPL-3.0-or-later).

use std::os::fd::AsFd as _;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use calloop::generic::Generic;
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::desktop::Window;
use smithay::output::Output;
use smithay::reexports::wayland_server::Display;

use super::client::{Client, ClientId};
use crate::backend::{Backend, Headless};
use crate::config::Config;
use crate::state::{ClientState, State as HecateState};

pub struct Server {
    pub event_loop: EventLoop<'static, HecateState>,
    pub state: HecateState,
}

impl Server {
    fn new(config: Config) -> Self {
        let event_loop = EventLoop::try_new().unwrap();
        let display = Display::new().unwrap();
        let state = HecateState::new_with_socket(
            config,
            None,
            event_loop.handle(),
            event_loop.get_signal(),
            display,
            Backend::Headless(Headless::new()),
            false,
        )
        .unwrap();
        let mut state = state;
        // Animations finish at once unless a test drives the clock (`manual_clock`).
        state.hecate.wm.clock.instant = true;
        Server { event_loop, state }
    }

    fn dispatch(&mut self) {
        self.event_loop.dispatch(Duration::ZERO, &mut self.state).unwrap();
        self.state.refresh_and_flush_clients();
    }
}

pub struct Fixture {
    pub event_loop: EventLoop<'static, TestState>,
    pub handle: LoopHandle<'static, TestState>,
    pub state: TestState,
}

pub struct TestState {
    pub server: Server,
    pub clients: Vec<Client>,
}

impl TestState {
    pub fn client(&mut self, id: ClientId) -> &mut Client {
        self.clients.iter_mut().find(|c| c.id == id).unwrap()
    }
}

impl Fixture {
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    pub fn with_toml(toml: &str) -> Self {
        Self::with_config(crate::config::load::from_toml_str(toml).expect("test config must parse"))
    }

    pub fn with_config(config: Config) -> Self {
        let event_loop = EventLoop::try_new().unwrap();
        let handle = event_loop.handle();
        let server = Server::new(config);
        let fd = server.event_loop.as_fd().try_clone_to_owned().unwrap();
        handle
            .insert_source(
                Generic::new(fd, Interest::READ, Mode::Level),
                |_, _, state: &mut TestState| {
                    state.server.dispatch();
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();
        Fixture {
            event_loop,
            handle,
            state: TestState {
                server,
                clients: Vec::new(),
            },
        }
    }

    pub fn dispatch(&mut self) {
        self.event_loop.dispatch(Duration::ZERO, &mut self.state).unwrap();
        // The server's own idle work (refresh, redraws) runs every iteration.
        self.state.server.dispatch();
    }

    /// Freeze the animation clock so a test can step through animations.
    pub fn manual_clock(&mut self) {
        self.hecate()
            .hecate
            .wm
            .clock
            .set_manual(Duration::from_secs(1000));
    }

    pub fn advance_clock(&mut self, ms: u64) {
        self.hecate().hecate.wm.clock.advance(Duration::from_millis(ms));
    }

    pub fn hecate(&mut self) -> &mut HecateState {
        &mut self.state.server.state
    }

    pub fn add_output(&mut self, name: &str, size: (i32, i32)) -> Output {
        let HecateState { backend, hecate } = &mut self.state.server.state;
        backend.headless().add_output(hecate, name, size)
    }

    /// Enable software rendering and frame capture.
    ///
    /// Fails the test if EGL isn't available, unless `HECATE_SKIP_RENDER_TESTS` is set (then
    /// returns false and the test should return early).
    pub fn enable_rendering(&mut self) -> bool {
        let h = self.hecate().backend.headless();
        if let Err(err) = h.add_renderer() {
            if std::env::var_os("HECATE_SKIP_RENDER_TESTS").is_some() {
                eprintln!("skipping render test: no EGL renderer ({err:?})");
                return false;
            }
            panic!(
                "no EGL renderer for render tests ({err:?}); run inside `nix develop` (Mesa llvmpipe) \
                 or set HECATE_SKIP_RENDER_TESTS=1"
            );
        }
        h.capture = true;
        // The cursor would sit on top of whatever pixel is at the output centre.
        self.hecate().hecate.cursor_status = smithay::input::pointer::CursorImageStatus::Hidden;
        true
    }

    /// Map a window backed by a real shm buffer of exactly `size`.
    pub fn map_shm_window(
        &mut self,
        id: ClientId,
        size: (i32, i32),
        rgba: [u8; 4],
    ) -> wayland_client::protocol::wl_surface::WlSurface {
        let window = self.client(id).create_window();
        let surface = window.surface.clone();
        window.commit();
        self.double_roundtrip(id);
        let window = self.client(id).window(&surface);
        window.attach_shm(size.0, size.1, rgba);
        window.ack_last_and_commit();
        self.double_roundtrip(id);
        surface
    }

    pub fn add_client(&mut self) -> ClientId {
        let (server_end, client_end) = UnixStream::pair().unwrap();
        self.hecate()
            .hecate
            .display_handle
            .insert_client(server_end, Arc::new(ClientState::default()))
            .unwrap();
        let client = Client::new(client_end);
        let id = client.id;
        let fd = client.event_loop.as_fd().try_clone_to_owned().unwrap();
        self.handle
            .insert_source(
                Generic::new(fd, Interest::READ, Mode::Level),
                move |_, _, state: &mut TestState| {
                    state.client(id).dispatch();
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();
        self.state.clients.push(client);
        self.roundtrip(id);
        id
    }

    pub fn client(&mut self, id: ClientId) -> &mut Client {
        self.state.client(id)
    }

    pub fn roundtrip(&mut self, id: ClientId) {
        let data = self.state.client(id).send_sync();
        let mut n = 0;
        while !data.done.load(Ordering::Relaxed) {
            self.dispatch();
            n += 1;
            assert!(n < 10_000, "roundtrip never completed");
        }
    }

    pub fn double_roundtrip(&mut self, id: ClientId) {
        self.roundtrip(id);
        self.roundtrip(id);
    }

    /// Map a window of `size` with a solid colour. Returns its wl_surface (client side).
    pub fn map_window(
        &mut self,
        id: ClientId,
        size: (u16, u16),
        rgba: [u8; 4],
        app_id: Option<&str>,
    ) -> wayland_client::protocol::wl_surface::WlSurface {
        let window = self.client(id).create_window();
        if let Some(a) = app_id {
            window.set_app_id(a);
        }
        let surface = window.surface.clone();
        window.commit();
        self.double_roundtrip(id);
        let window = self.client(id).window(&surface);
        window.attach_color(rgba);
        window.set_size(size.0, size.1);
        window.ack_last_and_commit();
        self.double_roundtrip(id);
        surface
    }

    /// Re-commit a window at the size it was last configured to (if any) and settle.
    pub fn ack_and_resize(
        &mut self,
        id: ClientId,
        surface: &wayland_client::protocol::wl_surface::WlSurface,
        rgba: [u8; 4],
    ) {
        let window = self.client(id).window(surface);
        let size = window
            .configures_received
            .last()
            .map(|(_, c)| c.size)
            .unwrap_or((0, 0));
        if size.0 > 0 && size.1 > 0 {
            window.set_size(size.0 as u16, size.1 as u16);
        }
        window.attach_color(rgba);
        window.ack_last_and_commit();
        self.double_roundtrip(id);
    }

    /// All mapped windows in creation order.
    pub fn windows(&mut self) -> Vec<Window> {
        let mut v: Vec<Window> = self.hecate().hecate.wm.all_windows().cloned().collect();
        use crate::wm::WindowExt;
        v.sort_by_key(|w| w.wid());
        v
    }

    pub fn focused(&mut self) -> Option<Window> {
        self.hecate().focused_window()
    }

    /// Render all outputs now and return the frame for `output`.
    pub fn render(&mut self, output: &str) -> Frame {
        self.hecate().hecate.queue_redraw_all();
        let HecateState { backend, hecate } = &mut self.state.server.state;
        hecate.redraw_queued_outputs(backend);
        let outputs: Vec<_> = self.hecate().hecate.wm.outputs().cloned().collect();
        for o in outputs {
            self.hecate().post_repaint(&o);
        }
        let (bytes, w, h) = self
            .hecate()
            .backend
            .headless()
            .frames
            .get(output)
            .cloned()
            .expect("no frame captured");
        Frame { bytes, w, h }
    }
}

pub struct Frame {
    pub bytes: Vec<u8>,
    pub w: i32,
    pub h: i32,
}

impl Frame {
    pub fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        assert!(
            x >= 0 && y >= 0 && x < self.w && y < self.h,
            "pixel ({x}, {y}) out of frame"
        );
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.bytes[i],
            self.bytes[i + 1],
            self.bytes[i + 2],
            self.bytes[i + 3],
        ]
    }

    /// Colour close to `expected` (per channel tolerance, alpha ignored).
    pub fn assert_rgb(&self, x: i32, y: i32, expected: [u8; 3], what: &str) {
        let p = self.pixel(x, y);
        let close = p
            .iter()
            .zip(expected.iter())
            .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 3);
        assert!(
            close,
            "{what}: pixel at ({x}, {y}) is {:?}, expected {:?}",
            &p[..3],
            expected
        );
    }
}
