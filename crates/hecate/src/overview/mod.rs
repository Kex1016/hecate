//! The window overview (`toggle_overview`, Mod+W): every window of each output's active
//! workspace laid out in a live grid, drawn by the compositor.

pub mod layout;
pub mod render;

use std::collections::HashMap;
use std::time::Duration;

use smithay::desktop::Window;
use smithay::output::Output;
use smithay::utils::{Logical, Rectangle};

use crate::anim::Animation;
use crate::render::border;
use crate::render::label;
use crate::state::State;
use crate::wm::{WindowExt, Wm};
use layout::{Item, Slot};

/// One window in the overview.
pub struct Entry {
    pub window: Window,
    pub slot: Slot,
    /// The window's content rect when the overview opened (or when it joined).
    pub from: Rectangle<i32, Logical>,
}

#[derive(Default)]
pub struct Overview {
    pub open: bool,
    /// The opening or closing transition, while it runs.
    pub anim: Option<Animation>,
    pub filter: String,
    pub selected: Option<Window>,
    pub previous_focus: Option<Window>,
    /// Entries per output name, in reading order.
    pub per_output: HashMap<String, Vec<Entry>>,
}

impl Overview {
    /// How far the overview is shown: 1 when open and settled, 0 when closed; it runs
    /// 0 → 1 while opening and 1 → 0 while closing.
    pub fn progress(&self, now: Duration) -> f64 {
        match &self.anim {
            Some(a) => a.presence(now, !self.open),
            None if self.open => 1.0,
            None => 0.0,
        }
    }

    /// Open, or still animating closed.
    pub fn is_visible(&self, now: Duration) -> bool {
        self.open || self.anim.as_ref().is_some_and(|a| !a.is_done(now))
    }

    /// Whether `w` passes the typed filter.
    pub fn matches(&self, w: &Window) -> bool {
        filter_matches(&self.filter, &w.app_id(), &w.title())
    }

    pub fn entries(&self, output: &Output) -> &[Entry] {
        self.per_output
            .get(&output.name())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn contains(&self, w: &Window) -> bool {
        self.per_output.values().flatten().any(|e| e.window == *w)
    }

    /// Drop a finished transition, and the entries once closed.
    fn settle(&mut self, now: Duration) {
        if self.anim.as_ref().is_some_and(|a| a.is_done(now)) {
            self.anim = None;
        }
        if !self.open && self.anim.is_none() {
            self.per_output.clear();
        }
    }
}

/// Case-insensitive substring match over a window's short app name, its app id and its
/// title. An empty filter matches everything.
pub fn filter_matches(filter: &str, app_id: &str, title: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let f = filter.to_lowercase();
    [
        label::app_name(app_id),
        app_id.to_lowercase(),
        title.to_lowercase(),
    ]
    .iter()
    .any(|s| s.contains(&f))
}

impl State {
    /// The windows the overview shows on `output`: its active workspace's, not minimized,
    /// that are mostly on it.
    fn overview_windows(&self, output: &Output) -> Vec<Window> {
        let wm = &self.hecate.wm;
        wm.active_workspace(output)
            .map(|ws| {
                ws.windows
                    .iter()
                    .filter(|w| wm.output_for_window(w) == Some(output))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Where the grid goes on `output`: its usable area inside the gap, with a row kept
    /// free at the top for the filter prompt.
    pub(crate) fn overview_area(&self, output: &Output) -> Rectangle<i32, Logical> {
        let gap = self.hecate.config.overview.gap.max(0);
        let mut area = Wm::usable_area(output, &self.hecate.config);
        let top = gap + label::NOTCH_HEIGHT + gap;
        area.loc.x += gap;
        area.loc.y += top;
        area.size.w = (area.size.w - 2 * gap).max(1);
        area.size.h = (area.size.h - top - gap).max(1);
        area
    }

    pub fn open_overview(&mut self) {
        if self.hecate.overview.open || self.hecate.wm.is_locked() {
            return;
        }
        let outputs: Vec<Output> = self.hecate.wm.outputs().cloned().collect();
        if outputs.iter().all(|o| self.overview_windows(o).is_empty()) {
            return;
        }
        let focused = self.focused_window();
        let config = &self.hecate.config.animations;
        let anim = Animation::start(&self.hecate.wm.clock, config, &config.overview);
        let o = &mut self.hecate.overview;
        o.open = true;
        o.anim = anim;
        o.filter.clear();
        o.per_output.clear();
        o.previous_focus = focused.clone();
        self.relayout_overview();
        let selected = match focused {
            Some(w) if self.hecate.overview.contains(&w) => Some(w),
            _ => self.first_overview_match(),
        };
        self.hecate.overview.selected = selected;
        self.ipc_broadcast_event(hecate_ipc::Event::OverviewChanged { open: true });
        self.hecate.queue_redraw_all();
    }

    /// Close the overview: focus `focus`, or the window that had focus before it opened.
    pub fn close_overview(&mut self, focus: Option<Window>) {
        if !self.hecate.overview.open {
            return;
        }
        let config = &self.hecate.config.animations;
        let anim = Animation::start(&self.hecate.wm.clock, config, &config.overview);
        self.finish_close(anim);
        let previous = self.hecate.overview.previous_focus.take();
        match focus {
            Some(w) => self.activate_window(&w),
            None => {
                if let Some(w) = previous.filter(|w| self.hecate.wm.all_windows().any(|x| x == w)) {
                    self.focus_window(&w);
                }
            }
        }
    }

    /// Close at once, with no transition and no refocus (the session locked).
    fn close_overview_now(&mut self) {
        if self.hecate.overview.open {
            self.finish_close(None);
            self.hecate.overview.previous_focus = None;
        }
    }

    fn finish_close(&mut self, anim: Option<Animation>) {
        let o = &mut self.hecate.overview;
        o.open = false;
        o.anim = anim;
        o.filter.clear();
        o.selected = None;
        if o.anim.is_none() {
            o.per_output.clear();
        }
        self.ipc_broadcast_event(hecate_ipc::Event::OverviewChanged { open: false });
        self.hecate.queue_redraw_all();
    }

    pub fn toggle_overview(&mut self) {
        if self.hecate.overview.open {
            self.close_overview(None);
        } else {
            self.open_overview();
        }
    }

    /// The first window passing the filter, in reading order, on the focused output first.
    pub(crate) fn first_overview_match(&self) -> Option<Window> {
        let o = &self.hecate.overview;
        let focused = self.hecate.wm.focused_output.as_ref().map(|o| o.name());
        let mut names: Vec<&String> = o.per_output.keys().collect();
        names.sort_by_key(|n| (Some(*n) != focused.as_ref(), (*n).clone()));
        names
            .into_iter()
            .flat_map(|n| &o.per_output[n])
            .map(|e| &e.window)
            .find(|w| o.matches(w))
            .cloned()
    }

    /// Lay every output's grid out again from the windows there now. Windows that stay
    /// keep where they came from; a selected window that left hands the selection to the
    /// next one in reading order.
    pub fn relayout_overview(&mut self) {
        let gap = self.hecate.config.overview.gap.max(0);
        let outputs: Vec<Output> = self.hecate.wm.outputs().cloned().collect();
        let old_order: Vec<(String, Vec<Window>)> = outputs
            .iter()
            .map(|o| {
                let ws = self.hecate.overview.entries(o).iter().map(|e| e.window.clone());
                (o.name(), ws.collect())
            })
            .collect();

        let mut per_output = HashMap::new();
        for output in &outputs {
            let windows = self.overview_windows(output);
            let old = self.hecate.overview.entries(output);
            let from: Vec<Rectangle<i32, Logical>> = windows
                .iter()
                .map(|w| {
                    old.iter()
                        .find(|e| e.window == *w)
                        .map(|e| e.from)
                        .unwrap_or_else(|| self.hecate.wm.window_rect(w))
                })
                .collect();
            let items: Vec<Item> = windows
                .iter()
                .zip(&from)
                .map(|(w, rect)| Item {
                    rect: *rect,
                    extents: border::extents_for(&self.hecate.config, w),
                })
                .collect();
            let slots = layout::grid(self.overview_area(output), &items, gap);
            let mut entries: Vec<Entry> = windows
                .into_iter()
                .zip(slots)
                .zip(from)
                .map(|((window, slot), from)| Entry { window, slot, from })
                .collect();
            entries.sort_by_key(|e| (e.slot.frame.loc.y, e.slot.frame.loc.x));
            per_output.insert(output.name(), entries);
        }
        self.hecate.overview.per_output = per_output;

        let o = &self.hecate.overview;
        if let Some(sel) = o.selected.clone()
            && !o.contains(&sel)
        {
            let next = old_order.iter().find_map(|(_, list)| {
                let i = list.iter().position(|w| *w == sel)?;
                let alive = |w: &&Window| o.contains(w) && o.matches(w);
                list[i + 1..]
                    .iter()
                    .find(alive)
                    .or_else(|| list[..i].iter().rev().find(alive))
                    .cloned()
            });
            let next = next.or_else(|| self.first_overview_match());
            self.hecate.overview.selected = next;
        }
        self.hecate.queue_redraw_all();
    }

    /// Once per loop iteration: close for a lock, follow windows coming and going and
    /// workspace switches, and drop a finished transition.
    pub(crate) fn refresh_overview(&mut self) {
        let now = self.hecate.wm.clock.now();
        if self.hecate.overview.open {
            if self.hecate.wm.is_locked() {
                self.close_overview_now();
            } else {
                let outputs: Vec<Output> = self.hecate.wm.outputs().cloned().collect();
                let stale = outputs.iter().any(|o| {
                    let now: Vec<Window> = self.overview_windows(o);
                    let shown: Vec<&Window> = self
                        .hecate
                        .overview
                        .entries(o)
                        .iter()
                        .map(|e| &e.window)
                        .collect();
                    now.len() != shown.len() || now.iter().any(|w| !shown.contains(&w))
                }) || self.hecate.overview.per_output.len() != outputs.len();
                if stale {
                    self.relayout_overview();
                }
            }
        }
        let was = self.hecate.overview.anim.is_some();
        self.hecate.overview.settle(now);
        if was && self.hecate.overview.anim.is_none() {
            self.hecate.queue_redraw_all();
        }
    }

    /// The session locked: the overview goes in the same frame.
    pub(crate) fn overview_locked(&mut self) {
        self.close_overview_now();
    }
}
