//! Wayland test client.
//!
//! Adapted from niri's test client (https://github.com/YaLTeR/niri, GPL-3.0-or-later).

use std::cmp::min;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1;
use smithay::reexports::wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1;
use smithay::reexports::wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibitor_v1::{self, ZwpKeyboardShortcutsInhibitorV1};
use smithay::reexports::wayland_protocols::wp::single_pixel_buffer;
use smithay::reexports::wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use smithay::reexports::wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{self, ZxdgToplevelDecorationV1};
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    self, ZwlrLayerShellV1,
};
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};
use smithay::reexports::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1};
use smithay::reexports::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1};
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1;
use smithay::reexports::wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1};
use smithay::reexports::wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1};
use smithay::reexports::wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1};
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1};
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1};
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1};
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1;
use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::client::zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::client::zwlr_output_power_v1::{self, ZwlrOutputPowerV1};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1;
use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_v1::{self, ZwlrOutputConfigurationV1};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::{self, ZwlrOutputHeadV1};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::{self, ZwlrOutputManagerV1};
use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{self, ZwlrOutputModeV1};
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_v1::{self, ZwlrGammaControlV1};
use smithay::reexports::wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use smithay::reexports::wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;
use wayland_backend::client::Backend;
use wayland_client::globals::Global;
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::{Connection, Dispatch, Proxy as _, QueueHandle};

pub struct Client {
    pub id: ClientId,
    pub event_loop: EventLoop<'static, State>,
    pub connection: Connection,
    pub qh: QueueHandle<State>,
    pub display: WlDisplay,
    pub state: State,
}

pub struct State {
    pub qh: QueueHandle<State>,

    pub globals: Vec<Global>,
    pub outputs: HashMap<WlOutput, String>,
    pub surfaces: HashMap<WlSurface, Surface>,
    pub seats: HashMap<WlSeat, Seat>,

    pub compositor: Option<WlCompositor>,
    pub shm: Option<WlShm>,
    pub decoration_manager: Option<ZxdgDecorationManagerV1>,
    /// Last decoration mode received per toplevel decoration object.
    pub decoration_modes: HashMap<ZxdgToplevelDecorationV1, zxdg_toplevel_decoration_v1::Mode>,
    pub xdg_wm_base: Option<XdgWmBase>,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub virtual_pointer_manager: Option<ZwlrVirtualPointerManagerV1>,
    pub spbm: Option<WpSinglePixelBufferManagerV1>,
    pub viewporter: Option<WpViewporter>,
    pub ksim: Option<ZwpKeyboardShortcutsInhibitManagerV1>,
    pub wlr_toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    pub ext_toplevel_list: Option<ExtForeignToplevelListV1>,
    /// Toplevels announced over wlr-foreign-toplevel-management, in announcement order.
    pub wlr_toplevels: Vec<(ZwlrForeignToplevelHandleV1, ForeignToplevel)>,
    /// Toplevels announced over ext-foreign-toplevel-list, in announcement order.
    pub ext_toplevels: Vec<(ExtForeignToplevelHandleV1, ForeignToplevel)>,
    pub workspace_manager: Option<ExtWorkspaceManagerV1>,
    pub ws_groups: Vec<(ExtWorkspaceGroupHandleV1, WsGroup)>,
    pub ws_handles: Vec<(ExtWorkspaceHandleV1, WsHandle)>,
    pub ws_done_count: usize,
    pub gamma_manager: Option<ZwlrGammaControlManagerV1>,
    pub output_manager: Option<ZwlrOutputManagerV1>,
    pub power_manager: Option<ZwlrOutputPowerManagerV1>,
    pub screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    pub output_source_manager: Option<ExtOutputImageCaptureSourceManagerV1>,
    pub toplevel_source_manager: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    pub copy_capture_manager: Option<ExtImageCopyCaptureManagerV1>,
    pub capture_sessions: Vec<(ExtImageCopyCaptureSessionV1, CaptureSession)>,
    pub capture_frames: Vec<(ExtImageCopyCaptureFrameV1, CopyFrame)>,
    pub screencopy_frames: Vec<(ZwlrScreencopyFrameV1, CopyFrame)>,
    /// Per power control: modes received in order, and whether it failed.
    pub power_controls: Vec<(ZwlrOutputPowerV1, Vec<zwlr_output_power_v1::Mode>, bool)>,
    pub output_serial: Option<u32>,
    pub heads: Vec<(ZwlrOutputHeadV1, Head)>,
    pub head_modes: HashMap<ZwlrOutputModeV1, HeadMode>,
    pub output_confs: HashMap<ZwlrOutputConfigurationV1, ConfResult>,
    /// Per gamma control: received size and whether it failed.
    pub gamma_controls: Vec<(ZwlrGammaControlV1, Option<u32>, bool)>,

    pub windows: Vec<Window>,
    pub layers: Vec<LayerSurface>,
}

pub struct Window {
    pub qh: QueueHandle<State>,
    pub spbm: WpSinglePixelBufferManagerV1,
    pub shm: WlShm,

    pub surface: WlSurface,
    pub xdg_surface: XdgSurface,
    pub xdg_toplevel: XdgToplevel,
    pub viewport: WpViewport,
    pub pending_configure: Configure,
    pub configures_received: Vec<(u32, Configure)>,
    pub close_requested: bool,
    pub last_acked: Option<u32>,

    pub configures_looked_at: usize,
}

pub struct LayerSurface {
    pub qh: QueueHandle<State>,
    pub spbm: WpSinglePixelBufferManagerV1,

    pub surface: WlSurface,
    pub layer_surface: ZwlrLayerSurfaceV1,
    pub viewport: WpViewport,
    pub configures_received: Vec<(u32, LayerConfigure)>,
    pub close_requested: bool,

    pub configures_looked_at: usize,
}

/// Committed (after `done`) and pending state of a foreign toplevel handle.
#[derive(Debug, Default, Clone)]
pub struct ForeignToplevel {
    pub identifier: String,
    pub title: String,
    pub app_id: String,
    pub states: Vec<u32>,
    pub outputs: usize,
    pub closed: bool,
    pub done_count: usize,
    pending: Option<Box<ForeignToplevel>>,
}

impl ForeignToplevel {
    fn pending(&mut self) -> &mut ForeignToplevel {
        if self.pending.is_none() {
            let mut p = self.clone();
            p.pending = None;
            self.pending = Some(Box::new(p));
        }
        self.pending.as_mut().unwrap()
    }

    fn done(&mut self) {
        if let Some(p) = self.pending.take() {
            let done_count = self.done_count;
            *self = *p;
            self.done_count = done_count;
        }
        self.done_count += 1;
    }

    pub fn activated(&self) -> bool {
        self.states
            .contains(&(zwlr_foreign_toplevel_handle_v1::State::Activated as u32))
    }

    pub fn has_state(&self, st: zwlr_foreign_toplevel_handle_v1::State) -> bool {
        self.states.contains(&(st as u32))
    }
}

/// Events received for a screencopy frame.
#[derive(Debug, Default, Clone)]
pub struct CopyFrame {
    /// (format, width, height, stride) of the shm buffer the compositor wants.
    pub shm: Option<(u32, u32, u32, u32)>,
    pub buffer_done: bool,
    pub damage: Vec<(u32, u32, u32, u32)>,
    pub ready: bool,
    pub failed: bool,
}

/// Events received on an ext-image-copy-capture session.
#[derive(Debug, Default, Clone)]
pub struct CaptureSession {
    pub buffer_size: Option<(u32, u32)>,
    pub shm_formats: Vec<u32>,
    pub done: usize,
    pub stopped: bool,
}

/// A client-side shm buffer we can read back.
pub struct CaptureBuffer {
    pub buffer: WlBuffer,
    file: std::fs::File,
    pub width: i32,
    pub height: i32,
    stride: i32,
}

impl CaptureBuffer {
    /// RGB at (x, y) of an XRGB8888 buffer.
    pub fn new(shm: &WlShm, qh: &QueueHandle<State>, w: i32, h: i32) -> Self {
        use std::os::fd::{AsFd, FromRawFd, OwnedFd};
        let stride = w * 4;
        let size = stride * h;
        // SAFETY: memfd_create with a valid NUL-terminated name.
        let fd = unsafe { libc::memfd_create(c"hecate-capture".as_ptr(), libc::MFD_CLOEXEC) };
        assert!(fd >= 0, "memfd_create failed");
        // SAFETY: fd is a fresh, owned descriptor.
        let file = std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        file.set_len(size as u64).unwrap();
        let pool: WlShmPool = shm.create_pool(file.as_fd(), size, qh, ());
        let buffer = pool.create_buffer(0, w, h, stride, wl_shm::Format::Xrgb8888, qh, ());
        pool.destroy();
        CaptureBuffer {
            buffer,
            file,
            width: w,
            height: h,
            stride,
        }
    }

    pub fn rgb(&self, x: i32, y: i32) -> [u8; 3] {
        use std::os::unix::fs::FileExt;
        let mut px = [0u8; 4];
        self.file
            .read_exact_at(&mut px, (y * self.stride + x * 4) as u64)
            .unwrap();
        // Little-endian B, G, R, X.
        [px[2], px[1], px[0]]
    }
}

#[derive(Debug, Default, Clone)]
pub struct Head {
    pub name: String,
    pub enabled: bool,
    pub modes: Vec<ZwlrOutputModeV1>,
    pub current_mode: Option<ZwlrOutputModeV1>,
    pub position: (i32, i32),
    pub scale: f64,
    pub transform: i32,
    pub adaptive_sync: bool,
    pub finished: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct HeadMode {
    pub size: (i32, i32),
    pub refresh: i32,
    pub preferred: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfResult {
    Pending,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Default, Clone)]
pub struct WsGroup {
    pub outputs: Vec<WlOutput>,
    pub workspaces: Vec<ExtWorkspaceHandleV1>,
    pub removed: bool,
}

#[derive(Debug, Default, Clone)]
pub struct WsHandle {
    pub id: Option<String>,
    pub name: String,
    pub coordinates: Vec<u32>,
    pub state: u32,
    pub capabilities: u32,
    pub removed: bool,
}

impl WsHandle {
    pub fn active(&self) -> bool {
        self.state & ext_workspace_handle_v1::State::Active.bits() != 0
    }

    pub fn urgent(&self) -> bool {
        self.state & ext_workspace_handle_v1::State::Urgent.bits() != 0
    }
}

#[derive(Debug, Default)]
pub struct Seat {
    pub keyboard: Option<Keyboard>,
}

#[derive(Debug)]
pub struct Surface {
    pub keyboard_events_received: Vec<KeyboardEvent>,
    pub keyboard_events_looked_at: usize,
}

#[derive(Debug)]
pub struct Keyboard {
    pub proxy: WlKeyboard,
    pub surface: Option<WlSurface>,
}

#[derive(Debug)]
pub enum KeyboardEvent {
    Enter {
        keys: Vec<u8>,
    },
    Leave,
    Key {
        key: u32,
        state: wl_keyboard::KeyState,
    },
    Modifiers {
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Configure {
    pub size: (i32, i32),
    pub bounds: Option<(i32, i32)>,
    pub states: Vec<xdg_toplevel::State>,
}

#[derive(Debug, Clone, Copy)]
pub struct LayerConfigure {
    pub size: (u32, u32),
}

#[derive(Clone, Copy, Default)]
pub struct LayerMargin {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Clone, Copy, Default)]
pub struct LayerConfigureProps {
    pub size: Option<(u32, u32)>,
    pub anchor: Option<zwlr_layer_surface_v1::Anchor>,
    pub exclusive_zone: Option<i32>,
    pub margin: Option<LayerMargin>,
    pub kb_interactivity: Option<zwlr_layer_surface_v1::KeyboardInteractivity>,
    pub layer: Option<zwlr_layer_shell_v1::Layer>,
    pub exclusive_edge: Option<zwlr_layer_surface_v1::Anchor>,
}

#[derive(Default)]
pub struct SyncData {
    pub done: AtomicBool,
}

static CLIENT_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u64);

impl ClientId {
    fn next() -> ClientId {
        ClientId(CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl fmt::Display for KeyboardEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyboardEvent::Enter { keys } => {
                write!(f, "enter: {keys:?}")?;
            }
            KeyboardEvent::Leave => {
                write!(f, "leave")?;
            }
            KeyboardEvent::Key { key, state } => {
                let action = match state {
                    wl_keyboard::KeyState::Released => "released",
                    wl_keyboard::KeyState::Pressed => "pressed",
                    wl_keyboard::KeyState::Repeated => "repeated",
                    _ => unreachable!(),
                };
                write!(f, "key {action}: {key}")?;
            }
            KeyboardEvent::Modifiers {
                depressed,
                latched,
                locked,
                group,
            } => {
                write!(
                    f,
                    "modifiers: depressed={depressed}, latched={latched}, \
                     locked={locked}, group={group}"
                )?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Configure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size: {} × {}, ", self.size.0, self.size.1)?;
        if let Some(bounds) = self.bounds {
            write!(f, "bounds: {} × {}, ", bounds.0, bounds.1)?;
        } else {
            write!(f, "bounds: none, ")?;
        }
        write!(f, "states: {:?}", self.states)?;
        Ok(())
    }
}

impl fmt::Display for LayerConfigure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size: {} × {}", self.size.0, self.size.1)?;
        Ok(())
    }
}

impl Client {
    pub fn new(stream: UnixStream) -> Self {
        let id = ClientId::next();

        let event_loop = EventLoop::try_new().unwrap();
        let backend = Backend::connect(stream).unwrap();
        let connection = Connection::from_backend(backend);
        let queue = connection.new_event_queue();
        let qh = queue.handle();
        WaylandSource::new(connection.clone(), queue)
            .insert(event_loop.handle())
            .unwrap();

        let display = connection.display();
        let _registry = display.get_registry(&qh, ());
        connection.flush().unwrap();

        let state = State {
            qh: qh.clone(),
            globals: Vec::new(),
            outputs: HashMap::new(),
            surfaces: HashMap::new(),
            seats: HashMap::new(),
            compositor: None,
            shm: None,
            decoration_manager: None,
            decoration_modes: HashMap::new(),
            xdg_wm_base: None,
            layer_shell: None,
            virtual_pointer_manager: None,
            spbm: None,
            viewporter: None,
            ksim: None,
            wlr_toplevel_manager: None,
            ext_toplevel_list: None,
            wlr_toplevels: Vec::new(),
            ext_toplevels: Vec::new(),
            workspace_manager: None,
            ws_groups: Vec::new(),
            ws_handles: Vec::new(),
            ws_done_count: 0,
            gamma_manager: None,
            output_manager: None,
            power_manager: None,
            screencopy_manager: None,
            output_source_manager: None,
            toplevel_source_manager: None,
            copy_capture_manager: None,
            capture_sessions: Vec::new(),
            capture_frames: Vec::new(),
            screencopy_frames: Vec::new(),
            power_controls: Vec::new(),
            output_serial: None,
            heads: Vec::new(),
            head_modes: HashMap::new(),
            output_confs: HashMap::new(),
            gamma_controls: Vec::new(),
            windows: Vec::new(),
            layers: Vec::new(),
        };

        Self {
            id,
            event_loop,
            connection,
            qh,
            display,
            state,
        }
    }

    pub fn dispatch(&mut self) {
        self.event_loop.dispatch(Duration::ZERO, &mut self.state).unwrap();

        if let Some(error) = self.connection.protocol_error() {
            panic!("{error}");
        }
    }

    pub fn send_sync(&self) -> Arc<SyncData> {
        let data = Arc::new(SyncData::default());
        self.display.sync(&self.qh, data.clone());
        self.connection.flush().unwrap();
        data
    }

    pub fn create_window(&mut self) -> &mut Window {
        self.state.create_window()
    }

    pub fn window(&mut self, surface: &WlSurface) -> &mut Window {
        self.state.window(surface)
    }

    pub fn create_layer(
        &mut self,
        output: Option<&WlOutput>,
        layer: zwlr_layer_shell_v1::Layer,
        namespace: &str,
    ) -> &mut LayerSurface {
        self.state.create_layer(output, layer, namespace.to_owned())
    }

    pub fn layer(&mut self, surface: &WlSurface) -> &mut LayerSurface {
        self.state.layer(surface)
    }

    pub fn output(&mut self, name: &str) -> WlOutput {
        self.state
            .outputs
            .iter()
            .find(|(_, v)| *v == name)
            .unwrap()
            .0
            .clone()
    }
}

impl State {
    pub fn create_surface(&mut self) -> WlSurface {
        let compositor = self.compositor.as_ref().unwrap();
        let surface = compositor.create_surface(&self.qh, ());
        self.surfaces.insert(
            surface.clone(),
            Surface {
                keyboard_events_received: Vec::new(),
                keyboard_events_looked_at: 0,
            },
        );
        surface
    }

    pub fn create_window(&mut self) -> &mut Window {
        let surface = self.create_surface();

        let xdg_wm_base = self.xdg_wm_base.as_ref().unwrap();
        let viewporter = self.viewporter.as_ref().unwrap();

        let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &self.qh, ());
        let xdg_toplevel = xdg_surface.get_toplevel(&self.qh, ());
        let viewport = viewporter.get_viewport(&surface, &self.qh, ());

        let window = Window {
            qh: self.qh.clone(),
            spbm: self.spbm.clone().unwrap(),
            shm: self.shm.clone().unwrap(),

            surface,
            xdg_surface,
            xdg_toplevel,
            viewport,
            pending_configure: Configure::default(),
            configures_received: Vec::new(),
            close_requested: false,
            last_acked: None,

            configures_looked_at: 0,
        };

        self.windows.push(window);
        self.windows.last_mut().unwrap()
    }

    pub fn window(&mut self, surface: &WlSurface) -> &mut Window {
        self.windows.iter_mut().find(|w| w.surface == *surface).unwrap()
    }

    pub fn create_layer(
        &mut self,
        output: Option<&WlOutput>,
        layer: zwlr_layer_shell_v1::Layer,
        namespace: String,
    ) -> &mut LayerSurface {
        let surface = self.create_surface();

        let layer_shell = self.layer_shell.as_ref().unwrap();
        let viewporter = self.viewporter.as_ref().unwrap();

        let layer_surface = layer_shell.get_layer_surface(&surface, output, layer, namespace, &self.qh, ());
        let viewport = viewporter.get_viewport(&surface, &self.qh, ());

        let layer_surface = LayerSurface {
            qh: self.qh.clone(),
            spbm: self.spbm.clone().unwrap(),

            surface,
            layer_surface,
            viewport,
            configures_received: Vec::new(),
            close_requested: false,

            configures_looked_at: 0,
        };

        self.layers.push(layer_surface);
        self.layers.last_mut().unwrap()
    }

    pub fn layer(&mut self, surface: &WlSurface) -> &mut LayerSurface {
        self.layers.iter_mut().find(|w| w.surface == *surface).unwrap()
    }

    pub fn recent_keyboard_events(
        &mut self,
        surface: &WlSurface,
    ) -> impl Iterator<Item = &KeyboardEvent> + '_ {
        let surface = self.surfaces.get_mut(surface).unwrap();
        surface.recent_keyboard_events()
    }

    pub fn inhibit_shortcuts(&self, surface: &WlSurface) -> ZwpKeyboardShortcutsInhibitorV1 {
        let ksim = self.ksim.as_ref().unwrap();
        let seat = self.seats.keys().next().unwrap();
        ksim.inhibit_shortcuts(surface, seat, &self.qh, ())
    }
}

/// 8-bit RGBA to the single-pixel-buffer's u32 channels (premultiplied by the caller).
fn spb_channels(rgba: [u8; 4]) -> [u32; 4] {
    rgba.map(|c| (c as u32) * (u32::MAX / 255))
}

impl Window {
    /// Attach a solid-colour shm buffer of `w`×`h` (goes through the texture shader path,
    /// unlike single-pixel buffers).
    pub fn attach_shm(&self, w: i32, h: i32, rgba: [u8; 4]) {
        let buffer = shm_buffer(&self.shm, &self.qh, w, h, rgba);
        self.viewport.set_destination(-1, -1);
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage_buffer(0, 0, w, h);
    }

    /// Attach a solid-colour single-pixel buffer (scaled by the viewport).
    pub fn attach_color(&self, rgba: [u8; 4]) {
        let [r, g, b, a] = spb_channels(rgba);
        let buffer = self.spbm.create_u32_rgba_buffer(r, g, b, a, &self.qh, ());
        self.surface.attach(Some(&buffer), 0, 0);
    }

    pub fn set_app_id(&self, app_id: &str) {
        self.xdg_toplevel.set_app_id(app_id.to_owned());
    }

    pub fn commit(&self) {
        self.surface.commit();
    }

    /// Ack the newest configure, unless it was already acked.
    pub fn ack_last(&mut self) {
        let serial = self.configures_received.last().unwrap().0;
        if self.last_acked != Some(serial) {
            self.xdg_surface.ack_configure(serial);
            self.last_acked = Some(serial);
        }
    }

    pub fn ack_last_and_commit(&mut self) {
        self.ack_last();
        self.commit();
    }

    pub fn attach_new_buffer(&self) {
        let buffer = self.spbm.create_u32_rgba_buffer(0, 0, 0, 0, &self.qh, ());
        self.surface.attach(Some(&buffer), 0, 0);
    }

    pub fn attach_null(&self) {
        self.surface.attach(None, 0, 0);
    }

    pub fn set_size(&self, w: u16, h: u16) {
        self.viewport.set_destination(i32::from(w), i32::from(h));
    }

    pub fn set_fullscreen(&self, output: Option<&WlOutput>) {
        self.xdg_toplevel.set_fullscreen(output);
    }

    pub fn unset_fullscreen(&self) {
        self.xdg_toplevel.unset_fullscreen();
    }

    pub fn set_maximized(&self) {
        self.xdg_toplevel.set_maximized();
    }

    pub fn unset_maximized(&self) {
        self.xdg_toplevel.unset_maximized();
    }

    pub fn set_parent(&self, parent: Option<&XdgToplevel>) {
        self.xdg_toplevel.set_parent(parent);
    }

    pub fn set_title(&self, title: &str) {
        self.xdg_toplevel.set_title(title.to_owned());
    }

    pub fn recent_configures(&mut self) -> impl Iterator<Item = &Configure> {
        let start = self.configures_looked_at;
        self.configures_looked_at = self.configures_received.len();
        self.configures_received[start..].iter().map(|(_, c)| c)
    }

    pub fn format_recent_configures(&mut self) -> String {
        let mut buf = String::new();
        for configure in self.recent_configures() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            write!(buf, "{configure}").unwrap();
        }
        buf
    }
}

impl LayerSurface {
    pub fn attach_color(&self, rgba: [u8; 4]) {
        let [r, g, b, a] = spb_channels(rgba);
        let buffer = self.spbm.create_u32_rgba_buffer(r, g, b, a, &self.qh, ());
        self.surface.attach(Some(&buffer), 0, 0);
    }

    pub fn commit(&self) {
        self.surface.commit();
    }

    pub fn ack_last(&self) {
        let serial = self.configures_received.last().unwrap().0;
        self.layer_surface.ack_configure(serial);
    }

    pub fn ack_last_and_commit(&self) {
        self.ack_last();
        self.commit();
    }

    pub fn set_configure_props(&self, props: LayerConfigureProps) {
        let LayerConfigureProps {
            size,
            anchor,
            exclusive_zone,
            margin,
            kb_interactivity,
            layer,
            exclusive_edge,
        } = props;

        if let Some(x) = size {
            self.layer_surface.set_size(x.0, x.1);
        }
        if let Some(x) = anchor {
            self.layer_surface.set_anchor(x);
        }
        if let Some(x) = exclusive_zone {
            self.layer_surface.set_exclusive_zone(x);
        }
        if let Some(x) = margin {
            self.layer_surface.set_margin(x.top, x.right, x.bottom, x.left);
        }
        if let Some(x) = kb_interactivity {
            self.layer_surface.set_keyboard_interactivity(x);
        }
        if let Some(x) = layer {
            self.layer_surface.set_layer(x);
        }
        if let Some(x) = exclusive_edge {
            self.layer_surface.set_exclusive_edge(x);
        }
    }

    pub fn attach_new_buffer(&self) {
        let buffer = self.spbm.create_u32_rgba_buffer(0, 0, 0, 0, &self.qh, ());
        self.surface.attach(Some(&buffer), 0, 0);
    }

    pub fn attach_null(&self) {
        self.surface.attach(None, 0, 0);
    }

    pub fn set_size(&self, w: u16, h: u16) {
        self.viewport.set_destination(i32::from(w), i32::from(h));
    }

    pub fn recent_configures(&mut self) -> impl Iterator<Item = &LayerConfigure> {
        let start = self.configures_looked_at;
        self.configures_looked_at = self.configures_received.len();
        self.configures_received[start..].iter().map(|(_, c)| c)
    }

    pub fn format_recent_configures(&mut self) -> String {
        let mut buf = String::new();
        for configure in self.recent_configures() {
            if !buf.is_empty() {
                buf.push('\n');
            }
            write!(buf, "{configure}").unwrap();
        }
        buf
    }
}

impl Surface {
    pub fn recent_keyboard_events(&mut self) -> impl Iterator<Item = &KeyboardEvent> {
        let start = self.keyboard_events_looked_at;
        self.keyboard_events_looked_at = self.keyboard_events_received.len();
        self.keyboard_events_received[start..].iter()
    }
}

impl Dispatch<WlCallback, Arc<SyncData>> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlCallback,
        event: <WlCallback as wayland_client::Proxy>::Event,
        data: &Arc<SyncData>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_callback::Event::Done { .. } => data.done.store(true, Ordering::Relaxed),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == ZxdgDecorationManagerV1::interface().name {
                    let version = min(version, ZxdgDecorationManagerV1::interface().version);
                    state.decoration_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == WlShm::interface().name {
                    let version = min(version, WlShm::interface().version);
                    state.shm = Some(registry.bind(name, version, qh, ()));
                } else if interface == WlCompositor::interface().name {
                    let version = min(version, WlCompositor::interface().version);
                    state.compositor = Some(registry.bind(name, version, qh, ()));
                } else if interface == XdgWmBase::interface().name {
                    let version = min(version, XdgWmBase::interface().version);
                    state.xdg_wm_base = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwlrLayerShellV1::interface().name {
                    let version = min(version, ZwlrLayerShellV1::interface().version);
                    state.layer_shell = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwlrVirtualPointerManagerV1::interface().name {
                    let version = min(version, ZwlrVirtualPointerManagerV1::interface().version);
                    state.virtual_pointer_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == WpSinglePixelBufferManagerV1::interface().name {
                    let version = min(version, WpSinglePixelBufferManagerV1::interface().version);
                    state.spbm = Some(registry.bind(name, version, qh, ()));
                } else if interface == WpViewporter::interface().name {
                    let version = min(version, WpViewporter::interface().version);
                    state.viewporter = Some(registry.bind(name, version, qh, ()));
                } else if interface == WlOutput::interface().name {
                    let version = min(version, WlOutput::interface().version);
                    let output = registry.bind(name, version, qh, ());
                    state.outputs.insert(output, String::new());
                } else if interface == WlSeat::interface().name {
                    let version = min(version, WlSeat::interface().version);
                    state
                        .seats
                        .insert(registry.bind(name, version, qh, ()), Seat::default());
                } else if interface == ZwlrForeignToplevelManagerV1::interface().name {
                    let version = min(version, ZwlrForeignToplevelManagerV1::interface().version);
                    state.wlr_toplevel_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ExtOutputImageCaptureSourceManagerV1::interface().name {
                    state.output_source_manager = Some(registry.bind(name, 1, qh, ()));
                } else if interface == ExtForeignToplevelImageCaptureSourceManagerV1::interface().name {
                    state.toplevel_source_manager = Some(registry.bind(name, 1, qh, ()));
                } else if interface == ExtImageCopyCaptureManagerV1::interface().name {
                    state.copy_capture_manager = Some(registry.bind(name, 1, qh, ()));
                } else if interface == ZwlrScreencopyManagerV1::interface().name {
                    let version = min(version, ZwlrScreencopyManagerV1::interface().version);
                    state.screencopy_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwlrOutputPowerManagerV1::interface().name {
                    let version = min(version, ZwlrOutputPowerManagerV1::interface().version);
                    state.power_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwlrOutputManagerV1::interface().name {
                    let version = min(version, ZwlrOutputManagerV1::interface().version);
                    state.output_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwlrGammaControlManagerV1::interface().name {
                    let version = min(version, ZwlrGammaControlManagerV1::interface().version);
                    state.gamma_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ExtWorkspaceManagerV1::interface().name {
                    let version = min(version, ExtWorkspaceManagerV1::interface().version);
                    state.workspace_manager = Some(registry.bind(name, version, qh, ()));
                } else if interface == ExtForeignToplevelListV1::interface().name {
                    let version = min(version, ExtForeignToplevelListV1::interface().version);
                    state.ext_toplevel_list = Some(registry.bind(name, version, qh, ()));
                } else if interface == ZwpKeyboardShortcutsInhibitManagerV1::interface().name {
                    let version = min(version, ZwpKeyboardShortcutsInhibitManagerV1::interface().version);
                    state.ksim = Some(registry.bind(name, version, qh, ()));
                }

                let global = Global {
                    name,
                    interface,
                    version,
                };
                state.globals.push(global);
            }
            wl_registry::Event::GlobalRemove { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry { .. } => (),
            wl_output::Event::Mode { .. } => (),
            wl_output::Event::Done => (),
            wl_output::Event::Scale { .. } => (),
            wl_output::Event::Name { name } => {
                *state.outputs.get_mut(output).unwrap() = name;
            }
            wl_output::Event::Description { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                let capabilities = capabilities.into_result().unwrap();
                let data = state.seats.get_mut(seat).unwrap();

                if capabilities.contains(wl_seat::Capability::Keyboard) {
                    if data.keyboard.is_none() {
                        let keyboard = Keyboard {
                            proxy: seat.get_keyboard(qh, seat.clone()),
                            surface: None,
                        };
                        data.keyboard = Some(keyboard);
                    }
                } else {
                    if let Some(keyboard) = data.keyboard.take() {
                        keyboard.proxy.release();
                    }
                }
            }
            wl_seat::Event::Name { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlKeyboard, WlSeat> for State {
    fn event(
        state: &mut Self,
        _keyboard: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        seat: &WlSeat,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let seat = state.seats.get_mut(seat).unwrap();
        let keyboard = seat.keyboard.as_mut().unwrap();

        match event {
            wl_keyboard::Event::Keymap { .. } => (),
            wl_keyboard::Event::Enter { surface, keys, .. } => {
                keyboard.surface = Some(surface.clone());

                let event = KeyboardEvent::Enter { keys };
                let surface = state.surfaces.get_mut(&surface).unwrap();
                surface.keyboard_events_received.push(event);
            }
            wl_keyboard::Event::Leave { .. } => {
                let surface = keyboard.surface.take().unwrap();

                let event = KeyboardEvent::Leave;
                let surface = state.surfaces.get_mut(&surface).unwrap();
                surface.keyboard_events_received.push(event);
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let key_state = key_state.into_result().unwrap();
                let surface = keyboard.surface.clone().unwrap();

                let event = KeyboardEvent::Key {
                    key,
                    state: key_state,
                };
                let surface = state.surfaces.get_mut(&surface).unwrap();
                surface.keyboard_events_received.push(event);
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                let surface = keyboard.surface.clone().unwrap();

                let event = KeyboardEvent::Modifiers {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                };
                let surface = state.surfaces.get_mut(&surface).unwrap();
                surface.keyboard_events_received.push(event);
            }
            wl_keyboard::Event::RepeatInfo { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlCompositor,
        _event: <WlCompositor as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<XdgWmBase, ()> for State {
    fn event(
        _state: &mut Self,
        xdg_wm_base: &XdgWmBase,
        event: <XdgWmBase as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            xdg_wm_base::Event::Ping { serial } => {
                xdg_wm_base.pong(serial);
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrLayerShellV1,
        _event: <ZwlrLayerShellV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

wayland_client::delegate_noop!(State: ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(State: ZwlrVirtualPointerV1);

impl Dispatch<WlSurface, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::Enter { .. } => (),
            wl_surface::Event::Leave { .. } => (),
            wl_surface::Event::PreferredBufferScale { .. } => (),
            wl_surface::Event::PreferredBufferTransform { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        xdg_surface: &XdgSurface,
        event: <XdgSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            xdg_surface::Event::Configure { serial } => {
                let window = state
                    .windows
                    .iter_mut()
                    .find(|w| w.xdg_surface == *xdg_surface)
                    .unwrap();
                let configure = window.pending_configure.clone();
                window.configures_received.push((serial, configure));
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<XdgToplevel, ()> for State {
    fn event(
        state: &mut Self,
        xdg_toplevel: &XdgToplevel,
        event: <XdgToplevel as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let window = state
            .windows
            .iter_mut()
            .find(|w| w.xdg_toplevel == *xdg_toplevel)
            .unwrap();

        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                let configure = &mut window.pending_configure;
                configure.size = (width, height);
                configure.states = states
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_ne_bytes(*c))
                    .flat_map(xdg_toplevel::State::try_from)
                    .collect();
            }
            xdg_toplevel::Event::Close => {
                window.close_requested = true;
            }
            xdg_toplevel::Event::ConfigureBounds { width, height } => {
                window.pending_configure.bounds = Some((width, height));
            }
            xdg_toplevel::Event::WmCapabilities { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        let layer_surface = state
            .layers
            .iter_mut()
            .find(|w| w.layer_surface == *layer_surface)
            .unwrap();

        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                let configure = LayerConfigure {
                    size: (width, height),
                };
                layer_surface.configures_received.push((serial, configure));
            }
            zwlr_layer_surface_v1::Event::Closed => layer_surface.close_requested = true,
            _ => unreachable!(),
        }
    }
}

/// Create an ARGB8888 shm buffer filled with one premultiplied colour.
fn shm_buffer(shm: &WlShm, qh: &QueueHandle<State>, w: i32, h: i32, rgba: [u8; 4]) -> WlBuffer {
    use std::io::Write as _;
    use std::os::fd::{AsFd, FromRawFd, OwnedFd};
    let stride = w * 4;
    let size = (stride * h) as usize;
    // SAFETY: memfd_create with a valid NUL-terminated name.
    let fd = unsafe { libc::memfd_create(c"hecate-test".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(fd >= 0, "memfd_create failed");
    // SAFETY: fd is a fresh, owned descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut file = std::fs::File::from(fd);
    let [r, g, b, a] = rgba;
    // ARGB8888 is little-endian B, G, R, A in memory.
    let px = [b, g, r, a];
    let mut data = Vec::with_capacity(size);
    for _ in 0..(w * h) {
        data.extend_from_slice(&px);
    }
    file.write_all(&data).unwrap();
    let pool: WlShmPool = shm.create_pool(file.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(0, w, h, stride, wl_shm::Format::Argb8888, qh, ());
    pool.destroy();
    buffer
}

wayland_client::delegate_noop!(State: ignore WlShm);
wayland_client::delegate_noop!(State: ignore ZxdgDecorationManagerV1);

impl Dispatch<ZxdgToplevelDecorationV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event
            && let Ok(mode) = mode.into_result()
        {
            state.decoration_modes.insert(proxy.clone(), mode);
        }
    }
}

impl State {
    /// Create an xdg-decoration object for a window and optionally request a mode.
    pub fn decorate(
        &mut self,
        surface: &WlSurface,
        request: Option<zxdg_toplevel_decoration_v1::Mode>,
    ) -> ZxdgToplevelDecorationV1 {
        let manager = self.decoration_manager.clone().expect("no xdg-decoration global");
        let toplevel = self.window(surface).xdg_toplevel.clone();
        let deco = manager.get_toplevel_decoration(&toplevel, &self.qh, ());
        if let Some(mode) = request {
            deco.set_mode(mode);
        }
        deco
    }
}
wayland_client::delegate_noop!(State: ignore WlShmPool);

impl Dispatch<WlBuffer, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_buffer::Event::Release => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WpSinglePixelBufferManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WpSinglePixelBufferManagerV1,
        _event: <WpSinglePixelBufferManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WpViewporter, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewporter,
        _event: <WpViewporter as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<WpViewport, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewport,
        _event: <WpViewport as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpKeyboardShortcutsInhibitManagerV1,
        _event: <ZwpKeyboardShortcutsInhibitManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        unreachable!()
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpKeyboardShortcutsInhibitorV1,
        event: <ZwpKeyboardShortcutsInhibitorV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Active => (),
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Inactive => (),
            _ => unreachable!(),
        }
    }
}

impl State {
    pub fn wlr_toplevel(&self, app_id: &str) -> Option<&(ZwlrForeignToplevelHandleV1, ForeignToplevel)> {
        self.wlr_toplevels
            .iter()
            .find(|(_, t)| !t.closed && t.app_id == app_id)
    }

    pub fn ext_toplevel(&self, app_id: &str) -> Option<&(ExtForeignToplevelHandleV1, ForeignToplevel)> {
        self.ext_toplevels
            .iter()
            .find(|(_, t)| !t.closed && t.app_id == app_id)
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                state.wlr_toplevels.push((toplevel, ForeignToplevel::default()));
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => (),
            _ => unreachable!(),
        }
    }

    wayland_client::event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, t)) = state.wlr_toplevels.iter_mut().find(|(h, _)| h == proxy) else {
            return;
        };
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => t.pending().title = title,
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => t.pending().app_id = app_id,
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                t.pending().states = state
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_ne_bytes(*c))
                    .collect();
            }
            zwlr_foreign_toplevel_handle_v1::Event::OutputEnter { .. } => t.pending().outputs += 1,
            zwlr_foreign_toplevel_handle_v1::Event::OutputLeave { .. } => {
                let p = t.pending();
                p.outputs = p.outputs.saturating_sub(1);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => t.done(),
            zwlr_foreign_toplevel_handle_v1::Event::Closed => t.closed = true,
            zwlr_foreign_toplevel_handle_v1::Event::Parent { .. } => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                state.ext_toplevels.push((toplevel, ForeignToplevel::default()));
            }
            ext_foreign_toplevel_list_v1::Event::Finished => (),
            _ => unreachable!(),
        }
    }

    wayland_client::event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, t)) = state.ext_toplevels.iter_mut().find(|(h, _)| h == proxy) else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                t.pending().identifier = identifier
            }
            ext_foreign_toplevel_handle_v1::Event::Title { title } => t.pending().title = title,
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => t.pending().app_id = app_id,
            ext_foreign_toplevel_handle_v1::Event::Done => t.done(),
            ext_foreign_toplevel_handle_v1::Event::Closed => t.closed = true,
            _ => unreachable!(),
        }
    }
}

impl State {
    /// Live (not removed) workspaces as (handle, state), in announcement order.
    pub fn live_workspaces(&self) -> Vec<(ExtWorkspaceHandleV1, WsHandle)> {
        self.ws_handles
            .iter()
            .filter(|(_, w)| !w.removed)
            .cloned()
            .collect()
    }

    pub fn live_ws_groups(&self) -> Vec<(ExtWorkspaceGroupHandleV1, WsGroup)> {
        self.ws_groups
            .iter()
            .filter(|(_, g)| !g.removed)
            .cloned()
            .collect()
    }

    pub fn workspace_named(&self, name: &str) -> Option<(ExtWorkspaceHandleV1, WsHandle)> {
        self.live_workspaces().into_iter().find(|(_, w)| w.name == name)
    }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
                state.ws_groups.push((workspace_group, WsGroup::default()));
            }
            ext_workspace_manager_v1::Event::Workspace { workspace } => {
                state.ws_handles.push((workspace, WsHandle::default()));
            }
            ext_workspace_manager_v1::Event::Done => state.ws_done_count += 1,
            ext_workspace_manager_v1::Event::Finished => (),
            _ => unreachable!(),
        }
    }

    wayland_client::event_created_child!(State, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, g)) = state.ws_groups.iter_mut().find(|(h, _)| h == proxy) else {
            return;
        };
        match event {
            ext_workspace_group_handle_v1::Event::Capabilities { .. } => (),
            ext_workspace_group_handle_v1::Event::OutputEnter { output } => g.outputs.push(output),
            ext_workspace_group_handle_v1::Event::OutputLeave { output } => {
                g.outputs.retain(|o| *o != output)
            }
            ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
                g.workspaces.push(workspace)
            }
            ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
                g.workspaces.retain(|w| *w != workspace)
            }
            ext_workspace_group_handle_v1::Event::Removed => g.removed = true,
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, w)) = state.ws_handles.iter_mut().find(|(h, _)| h == proxy) else {
            return;
        };
        match event {
            ext_workspace_handle_v1::Event::Id { id } => w.id = Some(id),
            ext_workspace_handle_v1::Event::Name { name } => w.name = name,
            ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
                w.coordinates = coordinates
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_ne_bytes(*c))
                    .collect();
            }
            ext_workspace_handle_v1::Event::State { state } => w.state = state.into(),
            ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
                w.capabilities = capabilities.into()
            }
            ext_workspace_handle_v1::Event::Removed => w.removed = true,
            _ => unreachable!(),
        }
    }
}

impl State {
    pub fn get_gamma_control(&mut self, output: &WlOutput) -> ZwlrGammaControlV1 {
        let control = self
            .gamma_manager
            .as_ref()
            .unwrap()
            .get_gamma_control(output, &self.qh, ());
        self.gamma_controls.push((control.clone(), None, false));
        control
    }

    /// (gamma_size, failed) for a control.
    pub fn gamma_status(&self, control: &ZwlrGammaControlV1) -> (Option<u32>, bool) {
        let (_, size, failed) = self.gamma_controls.iter().find(|(c, _, _)| c == control).unwrap();
        (*size, *failed)
    }
}

/// Send a ramp of raw u16 values to a gamma control through a memfd.
pub fn send_gamma(control: &ZwlrGammaControlV1, ramp: &[u16]) {
    use std::io::{Seek, Write};
    use std::os::fd::{AsFd, FromRawFd};
    // SAFETY: memfd_create returns a new owned fd or -1.
    let fd = unsafe { libc::memfd_create(c"hecate-gamma".as_ptr(), libc::MFD_CLOEXEC) };
    assert!(fd >= 0);
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let bytes: Vec<u8> = ramp.iter().flat_map(|v| v.to_ne_bytes()).collect();
    file.write_all(&bytes).unwrap();
    file.rewind().unwrap();
    control.set_gamma(file.as_fd());
}

wayland_client::delegate_noop!(State: ignore ZwlrGammaControlManagerV1);

impl Dispatch<ZwlrGammaControlV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrGammaControlV1,
        event: zwlr_gamma_control_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(entry) = state.gamma_controls.iter_mut().find(|(c, _, _)| c == proxy) else {
            return;
        };
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => entry.1 = Some(size),
            zwlr_gamma_control_v1::Event::Failed => entry.2 = true,
            _ => unreachable!(),
        }
    }
}

impl State {
    pub fn head(&self, name: &str) -> (ZwlrOutputHeadV1, Head) {
        self.heads
            .iter()
            .find(|(_, h)| !h.finished && h.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("no head {name}"))
    }

    pub fn head_mode(&self, mode: &ZwlrOutputModeV1) -> HeadMode {
        self.head_modes[mode]
    }

    /// Start a configuration at the last received serial.
    pub fn create_output_configuration(&mut self) -> ZwlrOutputConfigurationV1 {
        let serial = self.output_serial.expect("no output-management serial yet");
        let conf = self
            .output_manager
            .as_ref()
            .unwrap()
            .create_configuration(serial, &self.qh, ());
        self.output_confs.insert(conf.clone(), ConfResult::Pending);
        conf
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => state.heads.push((head, Head::default())),
            zwlr_output_manager_v1::Event::Done { serial } => state.output_serial = Some(serial),
            zwlr_output_manager_v1::Event::Finished => (),
            _ => unreachable!(),
        }
    }

    wayland_client::event_created_child!(State, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwlr_output_head_v1::Event::Mode { mode } = &event {
            state.head_modes.insert(mode.clone(), HeadMode::default());
        }
        let Some((_, h)) = state.heads.iter_mut().find(|(p, _)| p == proxy) else {
            return;
        };
        match event {
            zwlr_output_head_v1::Event::Name { name } => h.name = name,
            zwlr_output_head_v1::Event::Mode { mode } => h.modes.push(mode),
            zwlr_output_head_v1::Event::Enabled { enabled } => h.enabled = enabled != 0,
            zwlr_output_head_v1::Event::CurrentMode { mode } => h.current_mode = Some(mode),
            zwlr_output_head_v1::Event::Position { x, y } => h.position = (x, y),
            zwlr_output_head_v1::Event::Scale { scale } => h.scale = scale,
            zwlr_output_head_v1::Event::Transform { transform } => {
                h.transform = transform.into_result().map(|t| t as i32).unwrap_or(-1)
            }
            zwlr_output_head_v1::Event::AdaptiveSync { state } => {
                h.adaptive_sync = state
                    .into_result()
                    .is_ok_and(|s| s == zwlr_output_head_v1::AdaptiveSyncState::Enabled)
            }
            zwlr_output_head_v1::Event::Finished => h.finished = true,
            _ => (),
        }
    }

    wayland_client::event_created_child!(State, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let m = state.head_modes.entry(proxy.clone()).or_default();
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => m.size = (width, height),
            zwlr_output_mode_v1::Event::Refresh { refresh } => m.refresh = refresh,
            zwlr_output_mode_v1::Event::Preferred => m.preferred = true,
            zwlr_output_mode_v1::Event::Finished => (),
            _ => unreachable!(),
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputConfigurationV1,
        event: zwlr_output_configuration_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let result = match event {
            zwlr_output_configuration_v1::Event::Succeeded => ConfResult::Succeeded,
            zwlr_output_configuration_v1::Event::Failed => ConfResult::Failed,
            zwlr_output_configuration_v1::Event::Cancelled => ConfResult::Cancelled,
            _ => unreachable!(),
        };
        state.output_confs.insert(proxy.clone(), result);
    }
}

wayland_client::delegate_noop!(State: ignore ZwlrOutputConfigurationHeadV1);

impl State {
    pub fn get_output_power(&mut self, output: &WlOutput) -> ZwlrOutputPowerV1 {
        let c = self
            .power_manager
            .as_ref()
            .unwrap()
            .get_output_power(output, &self.qh, ());
        self.power_controls.push((c.clone(), Vec::new(), false));
        c
    }

    /// (modes received, failed)
    pub fn power_status(&self, c: &ZwlrOutputPowerV1) -> (Vec<zwlr_output_power_v1::Mode>, bool) {
        let (_, modes, failed) = self.power_controls.iter().find(|(x, _, _)| x == c).unwrap();
        (modes.clone(), *failed)
    }
}

wayland_client::delegate_noop!(State: ignore ZwlrOutputPowerManagerV1);

impl Dispatch<ZwlrOutputPowerV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputPowerV1,
        event: zwlr_output_power_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(entry) = state.power_controls.iter_mut().find(|(c, _, _)| c == proxy) else {
            return;
        };
        match event {
            zwlr_output_power_v1::Event::Mode { mode } => entry.1.push(mode.into_result().unwrap()),
            zwlr_output_power_v1::Event::Failed => entry.2 = true,
            _ => unreachable!(),
        }
    }
}

impl State {
    pub fn capture_output(&mut self, output: &WlOutput, cursor: bool) -> ZwlrScreencopyFrameV1 {
        let frame =
            self.screencopy_manager
                .as_ref()
                .unwrap()
                .capture_output(cursor as i32, output, &self.qh, ());
        self.screencopy_frames.push((frame.clone(), CopyFrame::default()));
        frame
    }

    pub fn capture_region(
        &mut self,
        output: &WlOutput,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> ZwlrScreencopyFrameV1 {
        let frame = self.screencopy_manager.as_ref().unwrap().capture_output_region(
            0,
            output,
            x,
            y,
            w,
            h,
            &self.qh,
            (),
        );
        self.screencopy_frames.push((frame.clone(), CopyFrame::default()));
        frame
    }

    pub fn copy_frame(&self, frame: &ZwlrScreencopyFrameV1) -> CopyFrame {
        self.screencopy_frames
            .iter()
            .find(|(f, _)| f == frame)
            .unwrap()
            .1
            .clone()
    }

    /// An XRGB8888 shm buffer matching what the compositor asked for.
    pub fn capture_buffer(&self, frame: &ZwlrScreencopyFrameV1) -> CaptureBuffer {
        let (_, w, h, stride) = self.copy_frame(frame).shm.expect("no buffer event");
        assert_eq!(stride, w * 4);
        CaptureBuffer::new(self.shm.as_ref().unwrap(), &self.qh, w as i32, h as i32)
    }
}

wayland_client::delegate_noop!(State: ignore ZwlrScreencopyManagerV1);

impl Dispatch<ZwlrScreencopyFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, f)) = state.screencopy_frames.iter_mut().find(|(p, _)| p == proxy) else {
            return;
        };
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format,
                width,
                height,
                stride,
            } => f.shm = Some((format.into(), width, height, stride)),
            zwlr_screencopy_frame_v1::Event::BufferDone => f.buffer_done = true,
            zwlr_screencopy_frame_v1::Event::Damage { x, y, width, height } => {
                f.damage.push((x, y, width, height))
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => f.ready = true,
            zwlr_screencopy_frame_v1::Event::Failed => f.failed = true,
            _ => (),
        }
    }
}

impl State {
    /// Start an ext-image-copy-capture session on an output.
    pub fn capture_session_output(&mut self, output: &WlOutput) -> ExtImageCopyCaptureSessionV1 {
        let source = self
            .output_source_manager
            .as_ref()
            .unwrap()
            .create_source(output, &self.qh, ());
        self.capture_session(&source)
    }

    /// Start an ext-image-copy-capture session on a toplevel from the ext list.
    pub fn capture_session_toplevel(
        &mut self,
        handle: &ExtForeignToplevelHandleV1,
    ) -> ExtImageCopyCaptureSessionV1 {
        let source = self
            .toplevel_source_manager
            .as_ref()
            .unwrap()
            .create_source(handle, &self.qh, ());
        self.capture_session(&source)
    }

    fn capture_session(&mut self, source: &ExtImageCaptureSourceV1) -> ExtImageCopyCaptureSessionV1 {
        let session = self.copy_capture_manager.as_ref().unwrap().create_session(
            source,
            ext_image_copy_capture_manager_v1::Options::empty(),
            &self.qh,
            (),
        );
        self.capture_sessions
            .push((session.clone(), CaptureSession::default()));
        session
    }

    pub fn capture_session_state(&self, s: &ExtImageCopyCaptureSessionV1) -> CaptureSession {
        self.capture_sessions
            .iter()
            .find(|(x, _)| x == s)
            .unwrap()
            .1
            .clone()
    }

    /// Create a frame into `buffer` and request capture.
    pub fn capture_frame(
        &mut self,
        s: &ExtImageCopyCaptureSessionV1,
        buffer: &CaptureBuffer,
    ) -> ExtImageCopyCaptureFrameV1 {
        let frame = s.create_frame(&self.qh, ());
        frame.attach_buffer(&buffer.buffer);
        frame.damage_buffer(0, 0, buffer.width, buffer.height);
        frame.capture();
        self.capture_frames.push((frame.clone(), CopyFrame::default()));
        frame
    }

    pub fn capture_frame_state(&self, f: &ExtImageCopyCaptureFrameV1) -> CopyFrame {
        self.capture_frames
            .iter()
            .find(|(x, _)| x == f)
            .unwrap()
            .1
            .clone()
    }
}

wayland_client::delegate_noop!(State: ignore ExtOutputImageCaptureSourceManagerV1);
wayland_client::delegate_noop!(State: ignore ExtForeignToplevelImageCaptureSourceManagerV1);
wayland_client::delegate_noop!(State: ignore ExtImageCaptureSourceV1);
wayland_client::delegate_noop!(State: ignore ExtImageCopyCaptureManagerV1);

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, s)) = state.capture_sessions.iter_mut().find(|(p, _)| p == proxy) else {
            return;
        };
        match event {
            ext_image_copy_capture_session_v1::Event::BufferSize { width, height } => {
                s.buffer_size = Some((width, height))
            }
            ext_image_copy_capture_session_v1::Event::ShmFormat { format } => {
                s.shm_formats.push(format.into())
            }
            ext_image_copy_capture_session_v1::Event::Done => s.done += 1,
            ext_image_copy_capture_session_v1::Event::Stopped => s.stopped = true,
            _ => (),
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some((_, f)) = state.capture_frames.iter_mut().find(|(p, _)| p == proxy) else {
            return;
        };
        match event {
            ext_image_copy_capture_frame_v1::Event::Damage { x, y, width, height } => {
                f.damage.push((x as u32, y as u32, width as u32, height as u32))
            }
            ext_image_copy_capture_frame_v1::Event::Ready => f.ready = true,
            ext_image_copy_capture_frame_v1::Event::Failed { .. } => f.failed = true,
            _ => (),
        }
    }
}
