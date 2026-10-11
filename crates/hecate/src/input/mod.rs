//! Input handling: keyboard binds, pointer focus, mouse binds.

pub mod grabs;

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::desktop::{Window, WindowSurfaceType, layer_map_for_output};
use smithay::input::keyboard::{FilterResult, XkbConfig};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::pointer_constraints::{PointerConstraint, with_pointer_constraint};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::config::action::{Action, LayoutRef};
use crate::config::binds::{BindValue, MouseContext};
use crate::config::keys::{Modifiers, Trigger};
use crate::state::State;
use crate::wm::{WindowExt, Wm};

/// What's under a point.
#[derive(Debug, Clone, Default)]
pub struct PointContents {
    pub surface: Option<(WlSurface, Point<f64, Logical>)>,
    pub window: Option<Window>,
    /// Over a window's border (outside its geometry).
    pub on_border: bool,
}

/// What a key event resolved to inside the keyboard filter.
enum KeyResult {
    Bind(BindValue, bool),
    Swallow,
    /// Ctrl+Alt+Fn: switch virtual terminal.
    Vt(i32),
    CancelPick,
}

impl State {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        self.hecate.idle_notifier_state.notify_activity(&self.hecate.seat);
        match event {
            InputEvent::Keyboard { event } => self.on_keyboard::<I>(event),
            InputEvent::PointerMotion { event } => self.on_pointer_motion::<I>(event),
            InputEvent::PointerMotionAbsolute { event } => self.on_pointer_motion_absolute::<I>(event),
            InputEvent::PointerButton { event } => self.on_pointer_button::<I>(event),
            InputEvent::PointerAxis { event } => self.on_pointer_axis::<I>(event),
            InputEvent::DeviceAdded { device } => self.backend.on_device_added(&device, &self.hecate.config),
            _ => {}
        }
    }

    fn modifiers_now(&self) -> Modifiers {
        let kb = self.hecate.seat.get_keyboard().unwrap();
        Modifiers::from_xkb(&kb.modifier_state())
    }

    fn shortcuts_inhibited(&self) -> bool {
        let kb = self.hecate.seat.get_keyboard().unwrap();
        kb.current_focus().is_some_and(|f| {
            self.hecate
                .shortcut_inhibitors
                .contains(&crate::utils::root_surface(&f))
                || self.hecate.shortcut_inhibitors.contains(&f)
        })
    }

    fn on_keyboard<I: InputBackend>(&mut self, event: I::KeyboardKeyEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let time = Event::time(&event);
        let keycode = event.key_code();
        let state = event.state();
        let pressed = state == KeyState::Pressed;
        let raw_code = keycode.raw();
        let inhibited = self.shortcuts_inhibited();
        let locked = self.hecate.wm.is_locked();
        let keybinds_enabled = self.hecate.keybinds_enabled;
        let mod_key = self.hecate.mod_key;
        let binds = self.hecate.binds.clone();
        let was_suppressed = self.hecate.suppressed_keys.contains(&raw_code);
        let pending_tap = self.hecate.pending_mod_tap.take();

        let picking = self.has_pending_pick();
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        let mut key_report: Option<hecate_ipc::KeyEvent> = None;
        let result = keyboard.input(self, keycode, state, serial, time, |_, mods, handle| {
            let modifiers = Modifiers::from_xkb(mods);
            {
                let sym = handle
                    .raw_latin_sym_or_raw_current_sym()
                    .unwrap_or_else(|| handle.modified_sym());
                let combo = crate::config::keys::KeyCombo {
                    modifiers,
                    trigger: Trigger::Keysym(sym),
                };
                key_report = Some(hecate_ipc::KeyEvent {
                    pressed,
                    keycode: raw_code,
                    keysym: smithay::input::keyboard::xkb::keysym_get_name(sym),
                    modifiers: combo
                        .to_string()
                        .split('+')
                        .rev()
                        .skip(1)
                        .map(String::from)
                        .collect(),
                    bind: combo.to_string(),
                    fired: None,
                });
            }
            if picking {
                if pressed && handle.modified_sym() == smithay::input::keyboard::Keysym::Escape {
                    return FilterResult::Intercept(KeyResult::CancelPick);
                }
                return FilterResult::Intercept(KeyResult::Swallow);
            }
            let sym = handle
                .raw_latin_sym_or_raw_current_sym()
                .unwrap_or_else(|| handle.modified_sym());
            let is_modifier_key = handle.modified_sym().is_modifier_key();

            if !pressed {
                // Modifier tap: the modifier released with nothing else pressed in between.
                if let Some(tap_mods) = pending_tap
                    && is_modifier_key
                {
                    let tap = binds.iter().find(|b| {
                        b.combo.trigger == Trigger::ModTap
                            && b.combo.modifiers.resolve(mod_key) == tap_mods
                            && b.value.tap.is_some()
                    });
                    if let Some(b) = tap
                        && (keybinds_enabled && (!inhibited || b.value.flags.allow_when_inhibited))
                        && (!locked || b.value.flags.allow_when_locked)
                    {
                        return FilterResult::Intercept(KeyResult::Bind(b.value.clone(), false));
                    }
                }
                // Release of a key whose press fired a bind.
                if was_suppressed {
                    let release = binds
                        .iter()
                        .find(|b| b.combo.trigger == Trigger::Keysym(sym) && b.value.on_release.is_some());
                    if let Some(b) = release {
                        return FilterResult::Intercept(KeyResult::Bind(b.value.clone(), false));
                    }
                    return FilterResult::Intercept(KeyResult::Swallow);
                }
                return FilterResult::Forward;
            }

            let modified = handle.modified_sym().raw();
            if (smithay::input::keyboard::keysyms::KEY_XF86Switch_VT_1
                ..=smithay::input::keyboard::keysyms::KEY_XF86Switch_VT_12)
                .contains(&modified)
            {
                let vt = (modified - smithay::input::keyboard::keysyms::KEY_XF86Switch_VT_1 + 1) as i32;
                return FilterResult::Intercept(KeyResult::Vt(vt));
            }

            for b in &binds {
                if b.combo.trigger != Trigger::Keysym(sym) {
                    continue;
                }
                // A modifier key bound on its own (e.g. Super_L hold) matches regardless of the
                // modifier state it itself sets.
                let want = b.combo.modifiers.resolve(mod_key);
                let have = if is_modifier_key && want.is_empty() {
                    Modifiers::empty()
                } else {
                    modifiers
                };
                if want != have {
                    continue;
                }
                let f = b.value.flags;
                if locked && !f.allow_when_locked {
                    continue;
                }
                if (!keybinds_enabled || inhibited) && !f.allow_when_inhibited {
                    continue;
                }
                return FilterResult::Intercept(KeyResult::Bind(b.value.clone(), true));
            }
            FilterResult::Forward
        });

        if let Some(mut report) = key_report {
            if let Some(KeyResult::Bind(value, _)) = &result {
                report.fired = value
                    .action
                    .as_ref()
                    .or(value.on_press.as_ref())
                    .or(value.tap.as_ref())
                    .and_then(|a| serde_json::to_string(a).ok());
            }
            self.ipc_broadcast_key(report);
        }

        // Track modifier taps: a lone modifier press arms it, anything else disarms.
        let is_mod_press = pressed && {
            let kb = self.hecate.seat.get_keyboard().unwrap();
            let mods = Modifiers::from_xkb(&kb.modifier_state());
            !mods.is_empty() && kb.pressed_keys().len() == 1
        };
        if is_mod_press {
            self.hecate.pending_mod_tap = Some(self.modifiers_now());
        }

        match result {
            Some(KeyResult::Bind(value, is_press)) => {
                if is_press {
                    self.hecate.suppressed_keys.push(raw_code);
                    if !value.flags.transparent {
                        self.hecate.pending_mod_tap = None;
                    }
                    let action = value.action.clone().or(value.on_press.clone());
                    if let Some(a) = action {
                        self.do_action(a, None);
                    }
                    if value.flags.transparent {
                        // Forward the key to the client as well.
                        let kb = self.hecate.seat.get_keyboard().unwrap();
                        kb.input_forward(self, keycode, state, serial, time, false);
                        self.hecate.suppressed_keys.retain(|k| *k != raw_code);
                    }
                } else {
                    self.hecate.suppressed_keys.retain(|k| *k != raw_code);
                    if let Some(a) = value.tap.clone().or(value.on_release.clone()) {
                        self.do_action(a, None);
                    }
                }
            }
            Some(KeyResult::Swallow) => {
                self.hecate.suppressed_keys.retain(|k| *k != raw_code);
            }
            Some(KeyResult::CancelPick) => {
                self.hecate.suppressed_keys.push(raw_code);
                self.finish_picks(true);
            }
            Some(KeyResult::Vt(vt)) => {
                self.hecate.suppressed_keys.push(raw_code);
                if let crate::backend::Backend::Tty(tty) = &mut self.backend {
                    tty.change_vt(vt);
                }
            }
            None => {}
        }
    }

    /// The topmost thing at a point, in render order.
    pub fn contents_under(&self, point: Point<f64, Logical>) -> PointContents {
        let mut out = PointContents::default();
        let Some(output) = self.hecate.wm.output_at(point).cloned() else {
            return out;
        };
        let output_geo = Wm::output_geometry(&output);

        if self.hecate.wm.is_locked() {
            if let Some(lock) = self.hecate.wm.lock_surface(&output) {
                let local = point - output_geo.loc.to_f64();
                if let Some((s, loc)) = smithay::desktop::utils::under_from_surface_tree(
                    lock.wl_surface(),
                    local,
                    (0, 0),
                    WindowSurfaceType::ALL,
                ) {
                    out.surface = Some((s, (loc + output_geo.loc).to_f64()));
                }
            }
            return out;
        }

        let layer_hit = |layer: Layer| -> Option<(WlSurface, Point<f64, Logical>)> {
            let map = layer_map_for_output(&output);
            let local = point - output_geo.loc.to_f64();
            let l = map.layer_under(layer, local)?;
            let lg = map.layer_geometry(l)?;
            let (s, loc) = l.surface_under(local - lg.loc.to_f64(), WindowSurfaceType::ALL)?;
            Some((s, (loc + lg.loc + output_geo.loc).to_f64()))
        };

        let fullscreen = self
            .hecate
            .wm
            .windows_on_output(&output)
            .iter()
            .any(|w| w.data().fullscreen);

        if let Some(hit) = layer_hit(Layer::Overlay) {
            out.surface = Some(hit);
            return out;
        }
        if !fullscreen && let Some(hit) = layer_hit(Layer::Top) {
            out.surface = Some(hit);
            return out;
        }
        if let Some((w, s, loc)) = self.hecate.wm.surface_under(point) {
            out.surface = Some((s, loc));
            out.window = Some(w);
            return out;
        }
        // Border area of a window.
        let bw = self.hecate.config.border.width.ceil() as i32;
        if bw > 0
            && let Some(w) = self.hecate.wm.window_at(point, bw)
            && !w.data().fullscreen
        {
            out.window = Some(w);
            out.on_border = true;
            return out;
        }
        if fullscreen && let Some(hit) = layer_hit(Layer::Top) {
            out.surface = Some(hit);
            return out;
        }
        if let Some(hit) = layer_hit(Layer::Bottom).or_else(|| layer_hit(Layer::Background)) {
            out.surface = Some(hit);
        }
        out
    }

    fn clamp_to_outputs(
        &self,
        old: Point<f64, Logical>,
        mut new: Point<f64, Logical>,
    ) -> Point<f64, Logical> {
        if self.hecate.wm.output_at(new).is_some() {
            return new;
        }
        let output = self
            .hecate
            .wm
            .output_at(old)
            .or_else(|| self.hecate.wm.outputs().next());
        if let Some(o) = output {
            let g = Wm::output_geometry(o).to_f64();
            new.x = new.x.clamp(g.loc.x, g.loc.x + g.size.w - 1.);
            new.y = new.y.clamp(g.loc.y, g.loc.y + g.size.h - 1.);
        }
        new
    }

    fn on_pointer_motion<I: InputBackend>(&mut self, event: I::PointerMotionEvent) {
        if self.hecate.wm.outputs().next().is_none() {
            return;
        }
        let pointer = self.hecate.seat.get_pointer().unwrap();
        let pos = pointer.current_location();
        let serial = SERIAL_COUNTER.next_serial();

        // Pointer constraints on the focused surface.
        let focus = self.contents_under(pos).surface;
        let mut locked = false;
        let mut confined: Option<(
            WlSurface,
            Point<f64, Logical>,
            Option<smithay::wayland::compositor::RegionAttributes>,
        )> = None;
        if let Some((surface, origin)) = &focus {
            with_pointer_constraint(surface, &pointer, |c| {
                let Some(c) = c else { return };
                if !c.is_active() {
                    return;
                }
                let local = pos - *origin;
                if let Some(region) = c.region()
                    && !region.contains(local.to_i32_round())
                {
                    return;
                }
                match &*c {
                    PointerConstraint::Locked(_) => locked = true,
                    PointerConstraint::Confined(conf) => {
                        confined = Some((surface.clone(), *origin, conf.region().cloned()));
                    }
                }
            });
        }
        let rel = RelativeMotionEvent {
            delta: event.delta(),
            delta_unaccel: event.delta_unaccel(),
            time: event.time(),
        };
        if locked {
            pointer.relative_motion(self, focus.clone(), &rel);
            pointer.frame(self);
            return;
        }

        let new_pos = self.clamp_to_outputs(pos, pos + event.delta());
        let under = self.contents_under(new_pos);

        if let Some((surface, origin, region)) = confined {
            let leaves_surface = under.surface.as_ref().map(|(s, _)| s) != Some(&surface);
            let leaves_region = region.is_some_and(|r| !r.contains((new_pos - origin).to_i32_round()));
            if leaves_surface || leaves_region {
                pointer.relative_motion(self, Some((surface, origin)), &rel);
                pointer.frame(self);
                return;
            }
        }

        self.focus_follows_mouse(&under);
        self.hecate.pointer_location = new_pos;
        pointer.motion(
            self,
            under.surface.clone(),
            &MotionEvent {
                location: new_pos,
                serial,
                time: event.time(),
            },
        );
        pointer.relative_motion(self, under.surface, &rel);
        pointer.frame(self);
        self.maybe_activate_constraint();
        self.update_focused_output_from_pointer();
        self.hecate.queue_redraw_all();
    }

    fn on_pointer_motion_absolute<I: InputBackend>(&mut self, event: I::PointerMotionAbsoluteEvent) {
        // Absolute events (winit, tablets) map onto the first output's area.
        let Some(output) = self.hecate.wm.outputs().next().cloned() else {
            return;
        };
        let geo = Wm::output_geometry(&output);
        let pos = event.position_transformed(geo.size) + geo.loc.to_f64();
        let serial = SERIAL_COUNTER.next_serial();
        let under = self.contents_under(pos);
        self.focus_follows_mouse(&under);
        self.hecate.pointer_location = pos;
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.motion(
            self,
            under.surface,
            &MotionEvent {
                location: pos,
                serial,
                time: event.time(),
            },
        );
        pointer.frame(self);
        self.update_focused_output_from_pointer();
        self.hecate.queue_redraw_all();
    }

    fn update_focused_output_from_pointer(&mut self) {
        if let Some(o) = self.hecate.wm.output_at(self.hecate.pointer_location).cloned()
            && self.hecate.wm.focused_output.as_ref() != Some(&o)
        {
            self.hecate.wm.focused_output = Some(o);
        }
    }

    fn maybe_activate_constraint(&mut self) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        let Some(focus) = pointer.current_focus() else {
            return;
        };
        let pos = pointer.current_location();
        let origin = self.contents_under(pos).surface.map(|(_, o)| o);
        with_pointer_constraint(&focus, &pointer, |c| {
            let Some(c) = c else { return };
            if c.is_active() {
                return;
            }
            if let (Some(region), Some(origin)) = (c.region(), origin)
                && !region.contains((pos - origin).to_i32_round())
            {
                return;
            }
            c.activate();
        });
    }

    fn focus_follows_mouse(&mut self, under: &PointContents) {
        if !self.hecate.config.focus.follow_mouse {
            return;
        }
        if self.hecate.seat.get_pointer().unwrap().is_grabbed() {
            return;
        }
        if let Some(w) = &under.window
            && self.focused_window().as_ref() != Some(w)
        {
            let raise = self.hecate.config.focus.raise_on_focus;
            self.hecate.config.focus.raise_on_focus = false;
            self.focus_window(w);
            self.hecate.config.focus.raise_on_focus = raise;
        }
    }

    pub fn warp_pointer(&mut self, pos: Point<f64, Logical>) {
        let pointer = self.hecate.seat.get_pointer().unwrap();
        let under = self.contents_under(pos);
        self.hecate.pointer_location = pos;
        pointer.motion(
            self,
            under.surface,
            &MotionEvent {
                location: pos,
                serial: SERIAL_COUNTER.next_serial(),
                time: smithay::backend::input::InputTime::now(),
            },
        );
        pointer.frame(self);
        self.hecate.queue_redraw_all();
    }

    fn on_pointer_button<I: InputBackend>(&mut self, event: I::PointerButtonEvent) {
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let state = event.state();
        let pointer = self.hecate.seat.get_pointer().unwrap();

        if state == ButtonState::Pressed && self.has_pending_pick() {
            self.hecate.suppressed_buttons.push(button);
            self.finish_picks(button != crate::config::keys::BTN_LEFT);
            return;
        }

        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            let pos = pointer.current_location();
            let under = self.contents_under(pos);
            let mods = self.modifiers_now();

            // Mouse binds.
            let context = if under.on_border {
                MouseContext::Border
            } else if under.window.is_some() {
                MouseContext::Window
            } else {
                MouseContext::Desktop
            };
            let bind = if self.hecate.keybinds_enabled && !self.hecate.wm.is_locked() {
                self.hecate
                    .mouse_binds
                    .iter()
                    .find(|(combo, mb)| {
                        combo.trigger == Trigger::Button(button)
                            && combo.modifiers.resolve(self.hecate.mod_key) == mods
                            && (mb.context == context || mb.context == MouseContext::Any)
                    })
                    .map(|(_, mb)| mb.action.clone())
            } else {
                None
            };

            if let Some(w) = &under.window {
                if self.focused_window().as_ref() != Some(w) {
                    self.focus_window(w);
                } else if self.hecate.config.focus.raise_on_focus {
                    self.hecate.wm.raise(w);
                }
            } else if let Some((surface, _)) = &under.surface {
                // Clicked a layer surface: give it focus if it wants keyboard input.
                self.focus_layer_if_wanted(surface);
            }

            if let Some(action) = bind {
                self.hecate.suppressed_buttons.push(button);
                match action {
                    Action::Move | Action::Resize => self.do_action(action, under.window.clone()),
                    other => self.do_action(other, under.window.clone()),
                }
                return;
            }
        }

        if state == ButtonState::Released && self.hecate.suppressed_buttons.contains(&button) {
            self.hecate.suppressed_buttons.retain(|b| *b != button);
            // Still deliver the release so grabs can end.
            pointer.button(
                self,
                &ButtonEvent {
                    button,
                    state,
                    serial,
                    time: event.time(),
                },
            );
            pointer.frame(self);
            return;
        }

        pointer.button(
            self,
            &ButtonEvent {
                button,
                state,
                serial,
                time: event.time(),
            },
        );
        pointer.frame(self);
    }

    fn focus_layer_if_wanted(&mut self, surface: &WlSurface) {
        let root = crate::utils::root_surface(surface);
        let target = self.hecate.wm.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            map.layer_for_surface(&root, WindowSurfaceType::TOPLEVEL)
                .filter(|l| l.can_receive_keyboard_focus())
                .map(|l| l.wl_surface().clone())
        });
        if let Some(s) = target {
            let kb = self.hecate.seat.get_keyboard().unwrap();
            kb.set_focus(self, Some(s), SERIAL_COUNTER.next_serial());
        }
    }

    fn on_pointer_axis<I: InputBackend>(&mut self, event: I::PointerAxisEvent) {
        let source = event.source();
        let h = event
            .amount(Axis::Horizontal)
            .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.) * 15. / 120.);
        let v = event
            .amount(Axis::Vertical)
            .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.) * 15. / 120.);

        // Wheel binds (discrete wheels only).
        if source == AxisSource::Wheel && self.hecate.keybinds_enabled && !self.hecate.wm.is_locked() {
            let mods = self.modifiers_now();
            let trigger = if v < 0. {
                Some(Trigger::WheelUp)
            } else if v > 0. {
                Some(Trigger::WheelDown)
            } else if h < 0. {
                Some(Trigger::WheelLeft)
            } else if h > 0. {
                Some(Trigger::WheelRight)
            } else {
                None
            };
            if let Some(t) = trigger
                && !mods.is_empty()
            {
                let bind = self
                    .hecate
                    .binds
                    .iter()
                    .find(|b| b.combo.trigger == t && b.combo.modifiers.resolve(self.hecate.mod_key) == mods)
                    .and_then(|b| b.value.action.clone());
                if let Some(a) = bind {
                    self.do_action(a, None);
                    return;
                }
            }
        }

        let mut frame = AxisFrame::new(event.time()).source(source);
        if h != 0. {
            frame = frame.value(Axis::Horizontal, h);
            if let Some(d) = event.amount_v120(Axis::Horizontal) {
                frame = frame.v120(Axis::Horizontal, d as i32);
            }
        }
        if v != 0. {
            frame = frame.value(Axis::Vertical, v);
            if let Some(d) = event.amount_v120(Axis::Vertical) {
                frame = frame.v120(Axis::Vertical, d as i32);
            }
        }
        if source == AxisSource::Finger {
            if event.amount(Axis::Horizontal) == Some(0.) {
                frame = frame.stop(Axis::Horizontal);
            }
            if event.amount(Axis::Vertical) == Some(0.) {
                frame = frame.stop(Axis::Vertical);
            }
        }
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    pub fn apply_keyboard_config(&mut self) {
        let kb = self.hecate.config.keyboard.clone();
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        let xkb = XkbConfig {
            rules: &kb.rules,
            model: &kb.model,
            layout: &kb.layout,
            variant: &kb.variant,
            options: if kb.options.is_empty() {
                None
            } else {
                Some(kb.options.clone())
            },
        };
        if let Err(err) = keyboard.set_xkb_config(self, xkb) {
            tracing::warn!("error applying keymap: {err:?}");
        }
        keyboard.change_repeat_info(kb.repeat_rate, kb.repeat_delay);
    }

    pub fn switch_layout(&mut self, r: LayoutRef) {
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        keyboard.with_xkb_state(self, |mut ctx| match &r {
            LayoutRef::Named(n) if n == "next" => ctx.cycle_next_layout(),
            LayoutRef::Named(n) if n == "prev" => ctx.cycle_prev_layout(),
            LayoutRef::Index(i) => ctx.set_layout(smithay::input::keyboard::Layout(*i)),
            LayoutRef::Named(name) => {
                let found = {
                    let xkb = ctx.xkb().lock().unwrap();
                    xkb.layouts().find(|l| xkb.layout_name(*l) == name)
                };
                if let Some(l) = found {
                    ctx.set_layout(l);
                }
            }
        });
    }

    /// Focused toplevel's surface or any keyboard focus.
    pub fn keyboard_focus(&self) -> Option<WlSurface> {
        self.hecate.seat.get_keyboard()?.current_focus()
    }

    pub fn pointer_focus_window(&self) -> Option<Window> {
        let pointer = self.hecate.seat.get_pointer()?;
        let focus = pointer.current_focus()?;
        self.hecate
            .window_for_surface(&crate::utils::root_surface(&focus))
    }
}
