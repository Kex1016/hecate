//! Keys and pointer while the overview is open.

use smithay::input::keyboard::Keysym;

use super::layout;
use crate::config::action::Direction;
use crate::config::keys::Modifiers;
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
}
