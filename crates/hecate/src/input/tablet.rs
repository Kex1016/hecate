//! Drawing tablets (tablet-v2): pens move the pointer and talk to the surface under them.

use smithay::backend::input::{
    AbsolutePositionEvent, Device, DeviceCapability, Event, InputBackend, ProximityState,
    TabletToolButtonEvent, TabletToolEvent, TabletToolProximityEvent, TabletToolTipEvent, TabletToolTipState,
};
use smithay::input::pointer::MotionEvent;
use smithay::input::tablet::{TabletDescriptor, TabletSeatTrait};
use smithay::output::Output;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::config::DeviceKind;
use crate::state::State;
use crate::wm::Wm;

impl State {
    /// The output an absolute device covers: `map_to_output` from its settings, else the
    /// focused output.
    pub(crate) fn absolute_output(&self, device_name: &str, kind: DeviceKind) -> Option<Output> {
        self.hecate
            .config
            .input_for(device_name, kind)
            .map_to_output
            .and_then(|name| self.hecate.wm.output_by_name(&name).cloned())
            .or_else(|| self.hecate.wm.focused_output.clone())
            .or_else(|| self.hecate.wm.outputs().next().cloned())
    }

    fn tablet_position<I: InputBackend, E: AbsolutePositionEvent<I>>(
        &self,
        event: &E,
    ) -> Option<Point<f64, Logical>> {
        let kind = DeviceKind {
            tablet: true,
            ..Default::default()
        };
        let output = self.absolute_output(&event.device().name(), kind)?;
        let geo = Wm::output_geometry(&output);
        Some(event.position_transformed(geo.size) + geo.loc.to_f64())
    }

    pub(super) fn on_tablet_added<Dev: Device>(&mut self, device: &Dev) {
        if device.has_capability(DeviceCapability::TabletTool) {
            self.hecate
                .seat
                .tablet_seat()
                .add_tablet(&TabletDescriptor::from(device));
        }
    }

    pub(super) fn on_tablet_removed<Dev: Device>(&mut self, device: &Dev) {
        if device.has_capability(DeviceCapability::TabletTool) {
            let tablet_seat = self.hecate.seat.tablet_seat();
            tablet_seat.remove_tablet(&TabletDescriptor::from(device));
            // No tablets left: forget the tools too.
            if tablet_seat.count_tablets() == 0 {
                tablet_seat.clear_tools();
            }
        }
    }

    /// Move the pointer along with the pen, and return what's under it.
    fn tablet_pointer_motion(
        &mut self,
        pos: Point<f64, Logical>,
        time: smithay::backend::input::InputTime,
    ) -> Option<(
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        Point<f64, Logical>,
    )> {
        let under = self.contents_under(pos).surface;
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.motion(
            self,
            under.clone(),
            &MotionEvent {
                location: pos,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
        pointer.frame(self);
        self.hecate.pointer_location = pos;
        self.hecate.queue_redraw_all();
        under
    }

    pub(super) fn on_tablet_axis<I: InputBackend>(&mut self, event: I::TabletToolAxisEvent) {
        use smithay::input::tablet::tool;
        let Some(pos) = self.tablet_position::<I, _>(&event) else {
            return;
        };
        let under = self.tablet_pointer_motion(pos, event.time());
        let Some(t) = self.hecate.seat.tablet_seat().get_tool(&event.tool()) else {
            return;
        };
        t.axis(
            self,
            tool::AxisFrame {
                pressure: event.pressure_has_changed().then(|| event.pressure()),
                distance: event.distance_has_changed().then(|| event.distance()),
                tilt: event.tilt_has_changed().then(|| event.tilt()),
                rotation: event.rotation_has_changed().then(|| event.rotation()),
                slider: event.slider_has_changed().then(|| event.slider_position()),
                wheel: event
                    .wheel_has_changed()
                    .then(|| (event.wheel_delta(), event.wheel_delta_discrete())),
            },
        );
        t.motion(
            self,
            under,
            &tool::MotionEvent {
                location: pos,
                serial: SERIAL_COUNTER.next_serial(),
                time: event.time(),
            },
        );
        t.frame(self, event.time());
    }

    pub(super) fn on_tablet_proximity<I: InputBackend>(&mut self, event: I::TabletToolProximityEvent) {
        use smithay::input::tablet::tool;
        let Some(pos) = self.tablet_position::<I, _>(&event) else {
            return;
        };
        let under = self.tablet_pointer_motion(pos, event.time());
        let dh = self.hecate.display_handle.clone();
        let tablet_seat = self.hecate.seat.tablet_seat();
        let Some(tablet) = tablet_seat.get_tablet(&TabletDescriptor::from(&event.device())) else {
            return;
        };
        let descriptor = event.tool();
        let t = match tablet_seat.get_tool(&descriptor) {
            Some(t) => t,
            None => tablet_seat.add_wp_tool(self, &dh, &descriptor),
        };
        match event.state() {
            ProximityState::In => t.proximity_in(
                self,
                under,
                tablet,
                &tool::ProximityInEvent {
                    location: pos,
                    axis: None,
                    serial: SERIAL_COUNTER.next_serial(),
                    time: event.time(),
                },
            ),
            ProximityState::Out => t.proximity_out(
                self,
                &tool::ProximityOutEvent {
                    serial: SERIAL_COUNTER.next_serial(),
                    time: event.time(),
                },
            ),
        }
        t.frame(self, event.time());
    }

    pub(super) fn on_tablet_tip<I: InputBackend>(&mut self, event: I::TabletToolTipEvent) {
        use smithay::input::tablet::tool;
        let Some(t) = self.hecate.seat.tablet_seat().get_tool(&event.tool()) else {
            return;
        };
        let serial = SERIAL_COUNTER.next_serial();
        match event.tip_state() {
            TabletToolTipState::Down => {
                t.down(
                    self,
                    &tool::DownEvent {
                        serial,
                        time: event.time(),
                    },
                );
                // Like a click: focus what's under the pen.
                let pos = self.hecate.pointer_location;
                let under = self.contents_under(pos);
                if let Some(w) = &under.window {
                    if self.focused_window().as_ref() != Some(w) {
                        self.focus_window(w);
                    }
                } else if let Some((s, _)) = &under.surface {
                    self.focus_layer_if_wanted(s);
                }
            }
            TabletToolTipState::Up => t.up(
                self,
                &tool::UpEvent {
                    serial,
                    time: event.time(),
                },
            ),
        }
        t.frame(self, event.time());
    }

    pub(super) fn on_tablet_button<I: InputBackend>(&mut self, event: I::TabletToolButtonEvent) {
        use smithay::input::tablet::tool;
        let Some(t) = self.hecate.seat.tablet_seat().get_tool(&event.tool()) else {
            return;
        };
        t.button(
            self,
            &tool::ButtonEvent {
                serial: SERIAL_COUNTER.next_serial(),
                button: event.button(),
                state: event.button_state(),
                time: event.time(),
            },
        );
        t.frame(self, event.time());
    }
}
