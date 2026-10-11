//! Touchpad gestures: horizontal swipes with the configured finger count switch workspaces;
//! every other gesture goes to the client under the pointer (pointer-gestures).

use smithay::backend::input::{
    Event, GestureBeginEvent, GestureEndEvent, GesturePinchUpdateEvent as _, GestureSwipeUpdateEvent as _,
    InputBackend, InputTime,
};
use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
    GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::config::action::{Action, WorkspaceRef};
use crate::state::State;

/// A swipe the compositor took for itself, and how far it has gone.
#[derive(Debug, Default)]
pub struct GestureState {
    swipe: Option<Point<f64, Logical>>,
}

impl crate::state::Hecate {
    /// No swipe is being taken by the compositor.
    pub fn gestures_idle(&self) -> bool {
        self.gestures.swipe.is_none()
    }
}

impl State {
    pub(super) fn on_swipe_begin<I: InputBackend>(&mut self, event: I::GestureSwipeBeginEvent) {
        self.swipe_begin(event.fingers(), event.time());
    }

    pub(super) fn on_swipe_update<I: InputBackend>(&mut self, event: I::GestureSwipeUpdateEvent) {
        self.swipe_update(event.delta(), event.time());
    }

    pub(super) fn on_swipe_end<I: InputBackend>(&mut self, event: I::GestureSwipeEndEvent) {
        self.swipe_end(event.cancelled(), event.time());
    }

    pub(super) fn on_pinch_begin<I: InputBackend>(&mut self, event: I::GesturePinchBeginEvent) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_pinch_begin(
            self,
            &GesturePinchBeginEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time: event.time(),
                fingers: event.fingers(),
            },
        );
    }

    pub(super) fn on_pinch_update<I: InputBackend>(&mut self, event: I::GesturePinchUpdateEvent) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_pinch_update(
            self,
            &GesturePinchUpdateEvent {
                time: event.time(),
                delta: event.delta(),
                scale: event.scale(),
                rotation: event.rotation(),
            },
        );
    }

    pub(super) fn on_pinch_end<I: InputBackend>(&mut self, event: I::GesturePinchEndEvent) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_pinch_end(
            self,
            &GesturePinchEndEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time: event.time(),
                cancelled: event.cancelled(),
            },
        );
    }

    pub(super) fn on_hold_begin<I: InputBackend>(&mut self, event: I::GestureHoldBeginEvent) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_hold_begin(
            self,
            &GestureHoldBeginEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time: event.time(),
                fingers: event.fingers(),
            },
        );
    }

    pub(super) fn on_hold_end<I: InputBackend>(&mut self, event: I::GestureHoldEndEvent) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_hold_end(
            self,
            &GestureHoldEndEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time: event.time(),
                cancelled: event.cancelled(),
            },
        );
    }

    pub fn swipe_begin(&mut self, fingers: u32, time: InputTime) {
        let ours = self.hecate.config.gestures.workspace_swipe_fingers;
        if ours != 0 && fingers == ours && !self.hecate.wm.is_locked() {
            self.hecate.gestures.swipe = Some(Point::default());
            return;
        }
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_swipe_begin(
            self,
            &GestureSwipeBeginEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time,
                fingers,
            },
        );
    }

    pub fn swipe_update(&mut self, delta: Point<f64, Logical>, time: InputTime) {
        if let Some(total) = &mut self.hecate.gestures.swipe {
            *total += delta;
            return;
        }
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_swipe_update(self, &GestureSwipeUpdateEvent { time, delta });
    }

    pub fn swipe_end(&mut self, cancelled: bool, time: InputTime) {
        if let Some(total) = self.hecate.gestures.swipe.take() {
            let g = &self.hecate.config.gestures;
            // Mostly horizontal and far enough.
            if !cancelled && total.x.abs() >= g.workspace_swipe_distance && total.x.abs() > total.y.abs() {
                // Fingers moving left bring in the workspace on the right, like a touch screen.
                let next = (total.x < 0.) != g.workspace_swipe_invert;
                let target = WorkspaceRef::Named(if next { "next" } else { "prev" }.into());
                self.do_action(Action::Workspace(target), None);
            }
            return;
        }
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.gesture_swipe_end(
            self,
            &GestureSwipeEndEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time,
                cancelled,
            },
        );
    }
}
