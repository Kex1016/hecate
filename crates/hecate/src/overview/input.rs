//! Keys and pointer while the overview is open.

use smithay::desktop::Window;
use smithay::input::keyboard::Keysym;
use smithay::utils::{Logical, Point, Rectangle};

use super::layout;
use crate::config::action::{Action, Direction};
use crate::config::keys::{BTN_LEFT, BTN_MIDDLE, Modifiers};
use crate::state::State;

impl State {
    /// A key pressed while the overview is open. `text` is the character it types, if any.
    pub(crate) fn overview_key(&mut self, sym: Keysym, mods: Modifiers, text: Option<char>) {
        if !self.hecate.overview.open {
            return;
        }
        let mod_key = Modifiers::MOD.resolve(self.hecate.mod_key);
        let commands = Modifiers::CTRL | Modifiers::ALT | Modifiers::SUPER | mod_key;
        let is_w = matches!(sym, Keysym::w | Keysym::W);
        match sym {
            Keysym::Escape => return self.close_overview(None),
            _ if is_w && mods.contains(mod_key) => return self.close_overview(None),
            Keysym::Return | Keysym::KP_Enter => {
                let o = &self.hecate.overview;
                if let Some(w) = o.selected.clone().filter(|w| o.matches(w)) {
                    self.close_overview(Some(w));
                }
                return;
            }
            Keysym::Left => return self.overview_step(Direction::Left),
            Keysym::Right => return self.overview_step(Direction::Right),
            Keysym::Up => return self.overview_step(Direction::Up),
            Keysym::Down => return self.overview_step(Direction::Down),
            Keysym::BackSpace => {
                self.hecate.overview.filter.pop();
            }
            Keysym::u | Keysym::U if mods.contains(Modifiers::CTRL) => {
                self.hecate.overview.filter.clear();
            }
            _ => match text {
                Some(c) if !c.is_control() && !mods.intersects(commands) => {
                    self.hecate.overview.filter.push(c);
                }
                _ => return,
            },
        }
        self.overview_filter_changed();
    }

    /// The filter changed: keep the selection on a match.
    fn overview_filter_changed(&mut self) {
        let o = &self.hecate.overview;
        if !o.selected.as_ref().is_some_and(|w| o.matches(w)) {
            self.hecate.overview.selected = self.first_overview_match();
        }
        self.hecate.queue_redraw_all();
    }

    /// Arrow keys: the next matching slot that way on the focused output.
    fn overview_step(&mut self, dir: Direction) {
        let Some(output) = self.hecate.wm.focused_output.clone() else {
            return;
        };
        let o = &self.hecate.overview;
        let entries = o.entries(&output);
        let eligible: Vec<bool> = entries.iter().map(|e| o.matches(&e.window)).collect();
        let current = o
            .selected
            .as_ref()
            .and_then(|s| entries.iter().position(|e| e.window == *s));
        let next = match current {
            Some(i) => {
                let slots: Vec<layout::Slot> = entries.iter().map(|e| e.slot).collect();
                layout::neighbour(&slots, i, dir, &eligible)
            }
            // Nothing selected here yet: start at the first match.
            None => eligible.iter().position(|&m| m),
        };
        if let Some(i) = next {
            self.hecate.overview.selected = Some(entries[i].window.clone());
            self.hecate.queue_redraw_all();
        }
    }

    /// What the pointer is over in the overview. Slots of windows the filter leaves out
    /// count as empty space.
    pub(crate) fn overview_hit(&self, pos: Point<f64, Logical>) -> Hit {
        let o = &self.hecate.overview;
        let inside = |r: Rectangle<i32, Logical>| r.to_f64().contains(pos);
        if let Some(sel) = &o.selected
            && let Some(e) = o.per_output.values().flatten().find(|e| e.window == *sel)
            && inside(layout::close_button(e.slot.frame))
        {
            return Hit::Close(sel.clone());
        }
        // The selected slot is drawn on top of the others.
        let mut entries: Vec<_> = o.per_output.values().flatten().collect();
        entries.sort_by_key(|e| o.selected.as_ref() != Some(&e.window));
        match entries.into_iter().find(|e| inside(e.slot.frame)) {
            Some(e) if o.matches(&e.window) => Hit::Slot(e.window.clone()),
            _ => Hit::Empty,
        }
    }

    /// A button pressed while the overview is open.
    pub(crate) fn overview_click(&mut self, pos: Point<f64, Logical>, button: u32) {
        if !self.hecate.overview.open {
            return;
        }
        match (button, self.overview_hit(pos)) {
            (BTN_LEFT, Hit::Slot(w)) => self.close_overview(Some(w)),
            (BTN_LEFT | BTN_MIDDLE, Hit::Close(w)) | (BTN_MIDDLE, Hit::Slot(w)) => {
                self.do_action(Action::Close, Some(w));
            }
            (BTN_LEFT, Hit::Empty) => self.close_overview(None),
            _ => {}
        }
    }

    /// The pointer moved while the overview is open: a matching slot under it is selected.
    pub(crate) fn overview_hover(&mut self, pos: Point<f64, Logical>) {
        if !self.hecate.overview.open {
            return;
        }
        let w = match self.overview_hit(pos) {
            Hit::Slot(w) | Hit::Close(w) => w,
            Hit::Empty => return,
        };
        if let Some(o) = self.hecate.wm.output_for_window(&w).cloned() {
            self.hecate.wm.focused_output = Some(o);
        }
        if self.hecate.overview.selected.as_ref() != Some(&w) {
            self.hecate.overview.selected = Some(w);
            self.hecate.queue_redraw_all();
        }
    }
}

impl State {
    /// A button from the input path. True when the overview took it: presses while it is
    /// open, and the releases of those presses.
    pub(crate) fn overview_button(&mut self, pos: Point<f64, Logical>, button: u32, pressed: bool) -> bool {
        if pressed {
            if !self.hecate.overview.open {
                return false;
            }
            self.hecate.suppressed_buttons.push(button);
            self.hecate.overview.pressed.push(button);
            self.overview_click(pos, button);
            true
        } else if self.hecate.suppressed_buttons.contains(&button) && self.overview_took(button) {
            self.hecate.suppressed_buttons.retain(|b| *b != button);
            self.hecate.overview.pressed.retain(|b| *b != button);
            true
        } else {
            false
        }
    }

    /// Whether the overview took the press of `button` (it records them while open).
    fn overview_took(&self, button: u32) -> bool {
        self.hecate.overview.pressed.contains(&button)
    }

    /// Pointer motion from the input path. While the overview is open it gets the motion
    /// and clients get none (their pointer focus goes).
    pub(crate) fn overview_motion(&mut self, pos: Point<f64, Logical>) -> bool {
        if !self.hecate.overview.open {
            return false;
        }
        self.hecate.pointer_location = pos;
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.motion(
            self,
            None,
            &smithay::input::pointer::MotionEvent {
                location: pos,
                serial: smithay::utils::SERIAL_COUNTER.next_serial(),
                time: smithay::backend::input::InputTime::now(),
            },
        );
        pointer.frame(self);
        if let Some(o) = self.hecate.wm.output_at(pos).cloned() {
            self.hecate.wm.focused_output = Some(o);
        }
        self.overview_hover(pos);
        self.hecate.queue_redraw_all();
        true
    }
}

/// What is under the pointer in the overview.
#[derive(Debug, Clone, PartialEq)]
pub enum Hit {
    Slot(Window),
    Close(Window),
    Empty,
}
