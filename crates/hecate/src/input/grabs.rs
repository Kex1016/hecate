//! Interactive move and resize.

use smithay::desktop::Window;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, CursorIcon, CursorImageStatus, Focus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData, MotionEvent,
    PointerGrab, PointerInnerHandle, RelativeMotionEvent,
};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, Rectangle, SERIAL_COUNTER, Serial};

use crate::config::action::Edge;
use crate::state::State;
use crate::wm::window::{Maximized, ResizeEdge, clamp_size};
use crate::wm::{WindowExt, Wm};

macro_rules! forward_gestures {
    () => {
        fn gesture_swipe_begin(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GestureSwipeBeginEvent,
        ) {
            h.gesture_swipe_begin(d, e)
        }
        fn gesture_swipe_update(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GestureSwipeUpdateEvent,
        ) {
            h.gesture_swipe_update(d, e)
        }
        fn gesture_swipe_end(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GestureSwipeEndEvent,
        ) {
            h.gesture_swipe_end(d, e)
        }
        fn gesture_pinch_begin(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GesturePinchBeginEvent,
        ) {
            h.gesture_pinch_begin(d, e)
        }
        fn gesture_pinch_update(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GesturePinchUpdateEvent,
        ) {
            h.gesture_pinch_update(d, e)
        }
        fn gesture_pinch_end(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GesturePinchEndEvent,
        ) {
            h.gesture_pinch_end(d, e)
        }
        fn gesture_hold_begin(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GestureHoldBeginEvent,
        ) {
            h.gesture_hold_begin(d, e)
        }
        fn gesture_hold_end(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            e: &GestureHoldEndEvent,
        ) {
            h.gesture_hold_end(d, e)
        }
        fn axis(&mut self, d: &mut State, h: &mut PointerInnerHandle<'_, State>, f: AxisFrame) {
            h.axis(d, f)
        }
        fn frame(&mut self, d: &mut State, h: &mut PointerInnerHandle<'_, State>) {
            h.frame(d)
        }
        fn relative_motion(
            &mut self,
            d: &mut State,
            h: &mut PointerInnerHandle<'_, State>,
            _focus: Option<(WlSurface, Point<f64, Logical>)>,
            e: &RelativeMotionEvent,
        ) {
            h.relative_motion(d, None, e)
        }
    };
}

fn grab_start(state: &State, serial: Option<Serial>) -> Option<PointerGrabStartData<State>> {
    let pointer = state.hecate.seat.get_pointer()?;
    match serial {
        // Client-initiated: the serial must match the button press that started it.
        Some(s) if pointer.has_grab(s) => pointer.grab_start_data(),
        Some(_) => None,
        // Compositor-initiated (mouse bind).
        None => Some(PointerGrabStartData {
            focus: None,
            button: 0,
            location: pointer.current_location(),
        }),
    }
}

/// Which edge of the window the pointer is closest to (for `Mod+right-drag` resizing).
pub fn edge_for_pointer(state: &State, window: &Window) -> ResizeEdge {
    let rect = state.hecate.wm.window_rect(window).to_f64();
    let p = state.hecate.pointer_location;
    let rx = (p.x - rect.loc.x) / rect.size.w.max(1.);
    let ry = (p.y - rect.loc.y) / rect.size.h.max(1.);
    let left = rx < 1. / 3.;
    let right = rx > 2. / 3.;
    let top = ry < 1. / 3.;
    let bottom = ry > 2. / 3.;
    match (left, right, top, bottom) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::BottomLeft,
        (_, true, true, _) => ResizeEdge::TopRight,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, _, _, _) => ResizeEdge::Left,
        (_, true, _, _) => ResizeEdge::Right,
        (_, _, true, _) => ResizeEdge::Top,
        (_, _, _, true) => ResizeEdge::Bottom,
        _ => ResizeEdge::BottomRight,
    }
}

pub fn start_move(state: &mut State, window: &Window, serial: Option<Serial>) {
    if window.data().fullscreen || window.data().rules.fixed_position {
        return;
    }
    let Some(start_data) = grab_start(state, serial) else {
        return;
    };
    let initial = state.hecate.wm.window_rect(window);
    let grab = MoveGrab {
        start_data,
        window: window.clone(),
        initial,
        unmaximized: false,
        snap: None,
    };
    let pointer = state.hecate.seat.get_pointer().unwrap();
    state.hecate.cursor_status = CursorImageStatus::Named(CursorIcon::Grabbing);
    pointer.set_grab(
        state,
        grab,
        serial.unwrap_or_else(|| SERIAL_COUNTER.next_serial()),
        Focus::Clear,
    );
}

pub fn start_resize(state: &mut State, window: &Window, edge: ResizeEdge, serial: Option<Serial>) {
    if edge == ResizeEdge::None || window.data().fullscreen {
        return;
    }
    let Some(start_data) = grab_start(state, serial) else {
        return;
    };
    if window.data().maximized != Maximized::No {
        state.set_maximized(window, Maximized::No);
    }
    let initial = state.hecate.wm.window_rect(window);
    {
        let mut d = window.data();
        d.resize_edge = Some(edge);
        d.resize_initial = Some(initial);
        d.resizing = true;
        d.snapped = None;
    }
    window
        .toplevel_surface()
        .with_pending_state(|s| s.states.set(xdg_toplevel::State::Resizing));
    let grab = ResizeGrab {
        start_data,
        window: window.clone(),
        edge,
        initial,
    };
    let pointer = state.hecate.seat.get_pointer().unwrap();
    state.hecate.cursor_status = CursorImageStatus::Named(resize_cursor(edge));
    pointer.set_grab(
        state,
        grab,
        serial.unwrap_or_else(|| SERIAL_COUNTER.next_serial()),
        Focus::Clear,
    );
}

fn resize_cursor(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top => CursorIcon::NResize,
        ResizeEdge::Bottom => CursorIcon::SResize,
        ResizeEdge::Left => CursorIcon::WResize,
        ResizeEdge::Right => CursorIcon::EResize,
        ResizeEdge::TopLeft => CursorIcon::NwResize,
        ResizeEdge::TopRight => CursorIcon::NeResize,
        ResizeEdge::BottomLeft => CursorIcon::SwResize,
        ResizeEdge::BottomRight => CursorIcon::SeResize,
        ResizeEdge::None => CursorIcon::Default,
    }
}

pub struct MoveGrab {
    start_data: PointerGrabStartData<State>,
    window: Window,
    initial: Rectangle<i32, Logical>,
    unmaximized: bool,
    snap: Option<Edge>,
}

/// Snap target when dragging near an output edge.
pub fn snap_zone(state: &State, pos: Point<f64, Logical>) -> Option<Edge> {
    let output = state.hecate.wm.output_at(pos)?;
    let area = Wm::usable_area(output, &state.hecate.config).to_f64();
    let range = state.hecate.config.snapping.range.inner.max(1) as f64;
    let corner = state.hecate.config.snapping.corner_range.max(1) as f64;
    let left = pos.x <= area.loc.x + range;
    let right = pos.x >= area.loc.x + area.size.w - range;
    let top = pos.y <= area.loc.y + range;
    let bottom = pos.y >= area.loc.y + area.size.h - range;
    let near_top = pos.y <= area.loc.y + corner;
    let near_bottom = pos.y >= area.loc.y + area.size.h - corner;
    let near_left = pos.x <= area.loc.x + corner;
    let near_right = pos.x >= area.loc.x + area.size.w - corner;
    Some(match () {
        _ if left && near_top || top && near_left => Edge::TopLeft,
        _ if right && near_top || top && near_right => Edge::TopRight,
        _ if left && near_bottom || bottom && near_left => Edge::BottomLeft,
        _ if right && near_bottom || bottom && near_right => Edge::BottomRight,
        _ if left => Edge::Left,
        _ if right => Edge::Right,
        _ if top => Edge::Top,
        _ if bottom => Edge::Bottom,
        _ => return None,
    })
}

impl PointerGrab<State> for MoveGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        if !self.window.alive() {
            handle.unset_grab(self, data, event.serial, event.time, true);
            return;
        }
        data.hecate.pointer_location = event.location;
        let delta = event.location - self.start_data.location;

        // Dragging a maximized/snapped window restores it under the pointer.
        let (maximized, snapped) = {
            let d = self.window.data();
            (d.maximized, d.snapped.is_some())
        };
        if !self.unmaximized
            && (maximized != Maximized::No || snapped)
            && (delta.x.abs() > 8. || delta.y.abs() > 8.)
        {
            self.unmaximized = true;
            let restore = self.window.data().restore;
            data.unsnap(&self.window);
            if let Some(r) = restore {
                // Keep the grab point proportionally inside the restored window.
                let rel = (self.start_data.location.x - self.initial.loc.x as f64)
                    / self.initial.size.w.max(1) as f64;
                self.initial = Rectangle::new(
                    (
                        (self.start_data.location.x - rel * r.size.w as f64) as i32,
                        self.initial.loc.y,
                    )
                        .into(),
                    r.size,
                );
            }
        }

        let mut loc = (self.initial.loc.to_f64() + delta).to_i32_round();
        loc = resist(data, &self.window, loc);
        self.window.data().loc = loc;
        self.snap = snap_zone(data, event.location);
        if let Some(o) = data.hecate.wm.output_at(event.location).cloned() {
            data.hecate.wm.focused_output = Some(o);
        }
        data.hecate.queue_redraw_all();
    }

    fn button(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, event: &ButtonEvent) {
        handle.button(data, event);
        if handle.current_pressed().is_empty() {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    forward_gestures!();

    fn start_data(&self) -> &PointerGrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        data.hecate.cursor_status = CursorImageStatus::default_named();
        if !self.window.alive() {
            return;
        }
        // In per_output scope, dropping onto another output moves the window to that output's
        // active workspace.
        let target = data.hecate.wm.output_for_window(&self.window).cloned();
        if let Some(o) = &target
            && data.hecate.wm.scope == crate::config::WorkspaceScope::PerOutput
        {
            let ws = data.hecate.wm.active_workspace_id(o);
            if self.window.data().workspace != ws {
                data.hecate.wm.move_window_to_workspace(&self.window, ws);
                // move_window_to_workspace translates; keep the dropped position.
            }
        }
        if let Some(edge) = self.snap.take() {
            data.snap_to_edge(&self.window, edge);
        }
        data.hecate.queue_redraw_all();
    }
}

/// Edge resistance: stick to output edges and other windows' edges within range.
fn resist(state: &State, window: &Window, loc: Point<i32, Logical>) -> Point<i32, Logical> {
    let screen = state.hecate.config.resistance.screen_edge;
    let win = state.hecate.config.resistance.window_edge;
    let size = window.geometry().size;
    let mut out = loc;
    let snap_axis = |value: i32, len: i32, edges: &[i32], range: i32| -> i32 {
        if range <= 0 {
            return value;
        }
        for &e in edges {
            if (value - e).abs() < range {
                return e;
            }
            if (value + len - e).abs() < range {
                return e - len;
            }
        }
        value
    };
    let center = Rectangle::new(loc, size).to_f64();
    let center = Point::from((
        center.loc.x + center.size.w / 2.,
        center.loc.y + center.size.h / 2.,
    ));
    if let Some(o) = state
        .hecate
        .wm
        .output_at(center)
        .or_else(|| state.hecate.wm.output_for_window(window))
    {
        let a = Wm::usable_area(o, &state.hecate.config);
        out.x = snap_axis(out.x, size.w, &[a.loc.x, a.loc.x + a.size.w], screen);
        out.y = snap_axis(out.y, size.h, &[a.loc.y, a.loc.y + a.size.h], screen);
        let others: Vec<Rectangle<i32, Logical>> = state
            .hecate
            .wm
            .windows_on_output(o)
            .iter()
            .filter(|w| *w != window)
            .map(|w| state.hecate.wm.window_rect(w))
            .collect();
        let xs: Vec<i32> = others
            .iter()
            .flat_map(|r| [r.loc.x, r.loc.x + r.size.w])
            .collect();
        let ys: Vec<i32> = others
            .iter()
            .flat_map(|r| [r.loc.y, r.loc.y + r.size.h])
            .collect();
        out.x = snap_axis(out.x, size.w, &xs, win);
        out.y = snap_axis(out.y, size.h, &ys, win);
    }
    out
}

pub struct ResizeGrab {
    start_data: PointerGrabStartData<State>,
    window: Window,
    edge: ResizeEdge,
    initial: Rectangle<i32, Logical>,
}

/// New size for a resize drag by `delta`, before min/max clamping.
pub fn resized(initial: Rectangle<i32, Logical>, edge: ResizeEdge, delta: Point<f64, Logical>) -> (i32, i32) {
    let mut w = initial.size.w as f64;
    let mut h = initial.size.h as f64;
    if edge.left() {
        w -= delta.x;
    } else if edge.right() {
        w += delta.x;
    }
    if edge.top() {
        h -= delta.y;
    } else if edge.bottom() {
        h += delta.y;
    }
    (w.round().max(1.) as i32, h.round().max(1.) as i32)
}

impl PointerGrab<State> for ResizeGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        if !self.window.alive() {
            handle.unset_grab(self, data, event.serial, event.time, true);
            return;
        }
        data.hecate.pointer_location = event.location;
        let delta = event.location - self.start_data.location;
        let (w, h) = resized(self.initial, self.edge, delta);
        let size = clamp_size(&self.window, (w, h).into());
        self.window.request_size(Some(size));
        data.hecate.queue_redraw_all();
    }

    fn button(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>, event: &ButtonEvent) {
        handle.button(data, event);
        if handle.current_pressed().is_empty() {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    forward_gestures!();

    fn start_data(&self) -> &PointerGrabStartData<State> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut State) {
        data.hecate.cursor_status = CursorImageStatus::default_named();
        self.window.data().resizing = false;
        let toplevel = self.window.toplevel_surface();
        toplevel.with_pending_state(|s| s.states.unset(xdg_toplevel::State::Resizing));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_math() {
        let r = Rectangle::new((100, 100).into(), (400, 300).into());
        assert_eq!(resized(r, ResizeEdge::BottomRight, (50., 20.).into()), (450, 320));
        assert_eq!(resized(r, ResizeEdge::TopLeft, (50., 20.).into()), (350, 280));
        assert_eq!(resized(r, ResizeEdge::Left, (-30., 999.).into()), (430, 300));
        assert_eq!(resized(r, ResizeEdge::Top, (999., 500.).into()), (400, 1));
    }
}
