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
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1};
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1};
use smithay::reexports::wayland_protocols::ext::workspace::v1::client::ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1};
use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1};
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
