//! Per-window state stored in the smithay `Window` user data.

use std::cell::{RefCell, RefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};

use crate::config::{BorderOverride, DecorationMode};

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Maximized {
    #[default]
    No,
    Both,
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    TopLeft,
    BottomLeft,
    Right,
    TopRight,
    BottomRight,
}

impl From<xdg_toplevel::ResizeEdge> for ResizeEdge {
    fn from(e: xdg_toplevel::ResizeEdge) -> Self {
        use xdg_toplevel::ResizeEdge as E;
        match e {
            E::Top => ResizeEdge::Top,
            E::Bottom => ResizeEdge::Bottom,
            E::Left => ResizeEdge::Left,
            E::TopLeft => ResizeEdge::TopLeft,
            E::BottomLeft => ResizeEdge::BottomLeft,
            E::Right => ResizeEdge::Right,
            E::TopRight => ResizeEdge::TopRight,
            E::BottomRight => ResizeEdge::BottomRight,
            _ => ResizeEdge::None,
        }
    }
}

impl ResizeEdge {
    pub fn left(self) -> bool {
        matches!(
            self,
            ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft
        )
    }
    pub fn right(self) -> bool {
        matches!(
            self,
            ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight
        )
    }
    pub fn top(self) -> bool {
        matches!(self, ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight)
    }
    pub fn bottom(self) -> bool {
        matches!(
            self,
            ResizeEdge::Bottom | ResizeEdge::BottomLeft | ResizeEdge::BottomRight
        )
    }
    pub fn to_xdg(self) -> xdg_toplevel::ResizeEdge {
        use xdg_toplevel::ResizeEdge as E;
        match self {
            ResizeEdge::None => E::None,
            ResizeEdge::Top => E::Top,
            ResizeEdge::Bottom => E::Bottom,
            ResizeEdge::Left => E::Left,
            ResizeEdge::TopLeft => E::TopLeft,
            ResizeEdge::BottomLeft => E::BottomLeft,
            ResizeEdge::Right => E::Right,
            ResizeEdge::TopRight => E::TopRight,
            ResizeEdge::BottomRight => E::BottomRight,
        }
    }
}

/// Snapped to a half or quarter of the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snap {
    Edge(crate::config::action::Edge),
}

#[derive(Debug)]
pub struct WindowData {
    pub id: u64,
    pub workspace: u64,
    /// Top-left of the window geometry in global logical coordinates.
    pub loc: Point<i32, Logical>,
    /// Outputs this window has entered (for wl_surface.enter/leave).
    pub entered_outputs: Vec<smithay::output::Output>,
    pub maximized: Maximized,
    pub fullscreen: bool,
    pub minimized: bool,
    pub always_on_top: bool,
    pub always_on_bottom: bool,
    pub omnipresent: bool,
    pub urgent: bool,
    pub snapped: Option<Snap>,
    /// Geometry to restore after unmaximize / unfullscreen / unsnap.
    pub restore: Option<Rectangle<i32, Logical>>,
    /// Interactive resize in progress: which edges move.
    pub resize_edge: Option<ResizeEdge>,
    /// Geometry when the resize started, to anchor the opposite edges.
    pub resize_initial: Option<Rectangle<i32, Logical>>,
    /// An interactive resize grab is active.
    pub resizing: bool,
    /// Index into the width/height presets for cycling.
    pub width_preset: Option<usize>,
    pub height_preset: Option<usize>,
    /// Effective window-rule properties.
    pub rules: ResolvedRules,
    pub border_hidden: bool,
    /// Last focus time, for MRU ordering.
    pub focus_serial: u64,
}

/// What window rules decided for this window.
#[derive(Debug, Clone, Default)]
pub struct ResolvedRules {
    pub border: Option<BorderOverride>,
    pub decorations: Option<DecorationMode>,
    pub tearing: bool,
    pub skip_taskbar: bool,
    pub ignore_focus_requests: bool,
    pub fixed_position: bool,
    pub no_animations: bool,
    pub opacity: Option<f32>,
    /// Indices of matched rules, in order.
    pub matched: Vec<usize>,
}

impl WindowData {
    fn new() -> Self {
        WindowData {
            id: NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed),
            workspace: 0,
            loc: Point::from((0, 0)),
            entered_outputs: Vec::new(),
            maximized: Maximized::No,
            fullscreen: false,
            minimized: false,
            always_on_top: false,
            always_on_bottom: false,
            omnipresent: false,
            urgent: false,
            snapped: None,
            restore: None,
            resize_edge: None,
            resize_initial: None,
            resizing: false,
            width_preset: None,
            height_preset: None,
            rules: ResolvedRules::default(),
            border_hidden: false,
            focus_serial: 0,
        }
    }
}

pub trait WindowExt {
    fn data(&self) -> RefMut<'_, WindowData>;
    fn wid(&self) -> u64;
    fn toplevel_surface(&self) -> &ToplevelSurface;
    fn app_id(&self) -> String;
    fn title(&self) -> String;
    fn min_size(&self) -> Size<i32, Logical>;
    fn max_size(&self) -> Size<i32, Logical>;
    /// Ask the client for a new size (and send the configure).
    fn request_size(&self, size: Option<Size<i32, Logical>>);
    fn set_activated_state(&self, activated: bool);
}

impl WindowExt for Window {
    fn data(&self) -> RefMut<'_, WindowData> {
        self.user_data()
            .get_or_insert(|| RefCell::new(WindowData::new()))
            .borrow_mut()
    }

    fn wid(&self) -> u64 {
        self.data().id
    }

    fn toplevel_surface(&self) -> &ToplevelSurface {
        self.toplevel().expect("hecate only manages xdg toplevels")
    }

    fn app_id(&self) -> String {
        with_states(self.toplevel_surface().wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|d| d.lock().unwrap().app_id.clone())
        })
        .unwrap_or_default()
    }

    fn title(&self) -> String {
        with_states(self.toplevel_surface().wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|d| d.lock().unwrap().title.clone())
        })
        .unwrap_or_default()
    }

    fn min_size(&self) -> Size<i32, Logical> {
        with_states(self.toplevel_surface().wl_surface(), |states| {
            states
                .cached_state
                .get::<smithay::wayland::shell::xdg::SurfaceCachedState>()
                .current()
                .min_size
        })
    }

    fn max_size(&self) -> Size<i32, Logical> {
        with_states(self.toplevel_surface().wl_surface(), |states| {
            states
                .cached_state
                .get::<smithay::wayland::shell::xdg::SurfaceCachedState>()
                .current()
                .max_size
        })
    }

    fn request_size(&self, size: Option<Size<i32, Logical>>) {
        let toplevel = self.toplevel_surface();
        toplevel.with_pending_state(|state| {
            state.size = size;
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn set_activated_state(&self, activated: bool) {
        if self.set_activated(activated) {
            let toplevel = self.toplevel_surface();
            if toplevel.is_initial_configure_sent() {
                toplevel.send_pending_configure();
            }
        }
    }
}

/// Clamp a requested size to the client's min/max hints.
pub fn clamp_size(window: &Window, size: Size<i32, Logical>) -> Size<i32, Logical> {
    let min = window.min_size();
    let max = window.max_size();
    let mut w = size.w.max(min.w.max(1));
    let mut h = size.h.max(min.h.max(1));
    if max.w > 0 {
        w = w.min(max.w);
    }
    if max.h > 0 {
        h = h.min(max.h);
    }
    (w, h).into()
}
