//! Touchscreens: touch points go to the surface under them, and a touch focuses its window.

use smithay::backend::input::{AbsolutePositionEvent, Device, Event, InputBackend, InputTime, TouchEvent};
use smithay::input::touch::{DownEvent, MotionEvent, UpEvent};
use smithay::output::Output;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

pub use smithay::backend::input::TouchSlot;

use crate::state::State;
use crate::wm::Wm;

impl State {
    /// The output a touch device maps to: `map_to_output` from its config, else the focused one.
    fn touch_output(&self, device_name: &str) -> Option<Output> {
        let kind = crate::config::DeviceKind {
            touch: true,
            ..Default::default()
        };
        self.hecate
            .config
            .input_for(device_name, kind)
            .map_to_output
            .and_then(|name| self.hecate.wm.output_by_name(&name).cloned())
            .or_else(|| self.hecate.wm.focused_output.clone())
            .or_else(|| self.hecate.wm.outputs().next().cloned())
    }

    fn touch_position<I: InputBackend, E: AbsolutePositionEvent<I>>(
        &self,
        event: &E,
    ) -> Option<Point<f64, Logical>> {
        let output = self.touch_output(&event.device().name())?;
        let geo = Wm::output_geometry(&output);
        Some(event.position_transformed(geo.size) + geo.loc.to_f64())
    }

    pub(super) fn on_touch_down<I: InputBackend>(&mut self, event: I::TouchDownEvent) {
        if let Some(pos) = self.touch_position::<I, _>(&event) {
            self.touch_down(event.slot(), pos, event.time());
        }
    }

    pub(super) fn on_touch_motion<I: InputBackend>(&mut self, event: I::TouchMotionEvent) {
        if let Some(pos) = self.touch_position::<I, _>(&event) {
            self.touch_motion(event.slot(), pos, event.time());
        }
    }

    pub(super) fn on_touch_up<I: InputBackend>(&mut self, event: I::TouchUpEvent) {
        self.touch_up(event.slot(), event.time());
    }

    pub fn touch_down(&mut self, slot: TouchSlot, pos: Point<f64, Logical>, time: InputTime) {
        let Some(touch) = self.hecate.seat.get_touch() else {
            return;
        };
        let under = self.contents_under(pos);
        if !touch.is_grabbed() && !self.hecate.wm.is_locked() {
            if let Some(w) = &under.window {
                if self.focused_window().as_ref() != Some(w) {
                    self.focus_window(w);
                }
            } else if let Some((surface, _)) = &under.surface {
                self.focus_layer_if_wanted(surface);
            }
        }
        touch.down(
            self,
            under.surface,
            &DownEvent {
                slot,
                location: pos,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }

    pub fn touch_motion(&mut self, slot: TouchSlot, pos: Point<f64, Logical>, time: InputTime) {
        let Some(touch) = self.hecate.seat.get_touch() else {
            return;
        };
        let under = self.contents_under(pos);
        touch.motion(
            self,
            under.surface,
            &MotionEvent {
                slot,
                location: pos,
                time,
            },
        );
    }

    pub fn touch_up(&mut self, slot: TouchSlot, time: InputTime) {
        let Some(touch) = self.hecate.seat.get_touch() else {
            return;
        };
        touch.up(
            self,
            &UpEvent {
                slot,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }

    pub fn touch_frame(&mut self) {
        if let Some(touch) = self.hecate.seat.get_touch() {
            touch.frame(self);
        }
    }

    pub fn touch_cancel(&mut self) {
        if let Some(touch) = self.hecate.seat.get_touch() {
            touch.cancel(self);
        }
    }
}
