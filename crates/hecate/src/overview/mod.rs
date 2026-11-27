//! The window overview (`toggle_overview`, Mod+W): every window of each output's active
//! workspace laid out in a live grid, drawn by the compositor.

pub mod input;
pub mod layout;
pub mod render;
pub mod strip;

use std::collections::HashMap;
use std::time::Duration;

use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
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
    /// Each output's usable area at the last layout (drawing can't look it up: the layer
    /// map is locked then).
    pub(crate) usable: HashMap<String, Rectangle<i32, Logical>>,
    /// The filter prompt's buffer, kept while its text and look stay the same.
    pub(crate) prompt: std::cell::RefCell<Option<(PromptKey, MemoryRenderBuffer, i32)>>,
    /// Each output's workspace strip (empty with `overview.strip = false`).
    pub strips: HashMap<String, Vec<strip::Thumb>>,
    /// A window being dragged towards the strip.
    pub drag: Option<strip::Drag>,
    /// The touch point that may become a tap: a left click where it went down, on release.
    pub touch: Option<Tap>,
    /// Touch points that went down while open. The overview keeps them to their release,
    /// also after it closed; ones that were down before stay with their client.
    pub(crate) touches: Vec<smithay::backend::input::TouchSlot>,
    /// Fills and lines of the strip, per workspace, kept for stable damage.
    pub(crate) solids:
        std::cell::RefCell<HashMap<(u64, u8), smithay::backend::renderer::element::solid::SolidColorBuffer>>,
    /// Ids of the strip's second drawing of each surface, per workspace thumbnail.
    pub(crate) ids: std::cell::RefCell<
        HashMap<(u64, smithay::backend::renderer::element::Id), smithay::backend::renderer::element::Id>,
    >,
    /// Workspace number labels of the strip.
    pub(crate) numbers: std::cell::RefCell<HashMap<(String, usize), MemoryRenderBuffer>>,
    /// Buttons pressed while open, whose releases the overview swallows too.
    pub(crate) pressed: Vec<u32>,
    /// The close button's buffer and what it was drawn from.
    pub(crate) close: std::cell::RefCell<Option<(PromptKey, MemoryRenderBuffer)>>,
}

/// A touch point that went down on the overview.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tap {
    pub slot: smithay::backend::input::TouchSlot,
    pub start: smithay::utils::Point<f64, Logical>,
    /// It went too far from `start` to be a tap.
    pub moved: bool,
}

/// What the filter prompt was drawn from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PromptKey {
    pub text: String,
    pub max_width: usize,
    pub fg: crate::config::Color,
    pub bg: crate::config::Color,
    pub zoom: usize,
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
            self.strips.clear();
            self.ids.borrow_mut().clear();
        }
    }
}

/// How an output's usable area is shared out: the strip on top, then the filter prompt's
/// row, then the grid, all inside the gap.
pub(crate) struct Rows {
    /// The strip's row (zero height without the strip).
    pub strip: Rectangle<i32, Logical>,
    /// The top of the filter prompt.
    pub prompt_y: i32,
    pub grid: Rectangle<i32, Logical>,
}

impl Rows {
    pub fn new(usable: Rectangle<i32, Logical>, gap: i32, strip: bool) -> Rows {
        let gap = gap.max(0);
        let inner = Rectangle::<i32, Logical>::new(
            (usable.loc.x + gap, usable.loc.y + gap).into(),
            ((usable.size.w - 2 * gap).max(1), (usable.size.h - 2 * gap).max(1)).into(),
        );
        let strip_h = if strip { strip::height(inner) } else { 0 };
        let strip_row = Rectangle::new(inner.loc, (inner.size.w, strip_h).into());
        let prompt_y = if strip {
            inner.loc.y + strip_h + gap
        } else {
            inner.loc.y
        };
        let grid_y = prompt_y + label::NOTCH_HEIGHT + gap;
        let bottom = inner.loc.y + inner.size.h;
        let grid = Rectangle::new(
            (inner.loc.x, grid_y).into(),
            (inner.size.w, (bottom - grid_y).max(1)).into(),
        );
        Rows {
            strip: Rectangle::new(strip_row.loc, (strip_row.size.w, inner.size.h).into()),
            prompt_y,
            grid,
        }
    }
}

/// The frame of a strip thumbnail.
pub(crate) const THUMB_EXTENTS: border::Extents = border::Extents {
    top: 2,
    right: 2,
    bottom: 2,
    left: 2,
};

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

    /// Where the grid goes on `output`: below the strip and the filter prompt's row.
    pub(crate) fn overview_area(&self, output: &Output) -> Rectangle<i32, Logical> {
        let c = &self.hecate.config.overview;
        Rows::new(Wm::usable_area(output, &self.hecate.config), c.gap, c.strip).grid
    }

    pub fn open_overview(&mut self) {
        // A layer surface with the keyboard to itself (a polkit prompt, the launcher) keeps
        // it: nothing typed there may end up in the filter.
        if self.hecate.overview.open || self.hecate.wm.is_locked() || self.keyboard_on_exclusive_layer() {
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
        // Clients lose the pointer: a scroll or click must not reach a hidden window.
        let pointer = self.hecate.seat.get_pointer().unwrap();
        pointer.motion(
            self,
            None,
            &smithay::input::pointer::MotionEvent {
                location: self.hecate.pointer_location,
                serial: smithay::utils::SERIAL_COUNTER.next_serial(),
                time: smithay::backend::input::InputTime::now(),
            },
        );
        pointer.frame(self);
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
            Some(w) => {
                // A picked window is shown, whatever hid the desktop, and comes to the
                // front even where focusing alone would not raise it.
                self.hecate.wm.show_desktop = false;
                self.activate_window(&w);
                self.hecate.wm.raise(&w);
            }
            None => {
                // Back to where focus was, unless the overview moved it or switched away
                // from its workspace: then the topmost window there.
                let shown = |w: &Window| {
                    let wm = &self.hecate.wm;
                    wm.workspace_of(w)
                        .is_some_and(|ws| wm.is_workspace_active(ws.id) && !w.data().minimized)
                };
                match previous.filter(|w| shown(w)) {
                    Some(w) => self.focus_window(&w),
                    None => self.focus_topmost(),
                }
            }
        }
        // What is under the pointer gets it back.
        self.warp_pointer(self.hecate.pointer_location);
    }

    /// Close at once, with no transition and no refocus (the session locked).
    fn close_overview_now(&mut self) {
        if self.hecate.overview.open {
            self.finish_close(None);
            self.hecate.overview.previous_focus = None;
            self.warp_pointer(self.hecate.pointer_location);
        }
    }

    /// Close and leave focus alone: an exclusive-keyboard layer surface has it.
    fn close_overview_for_layer(&mut self) {
        if self.hecate.overview.open {
            let config = &self.hecate.config.animations;
            let anim = Animation::start(&self.hecate.wm.clock, config, &config.overview);
            self.finish_close(anim);
            self.hecate.overview.previous_focus = None;
            self.warp_pointer(self.hecate.pointer_location);
        }
    }

    fn finish_close(&mut self, anim: Option<Animation>) {
        let o = &mut self.hecate.overview;
        o.open = false;
        o.anim = anim;
        o.filter.clear();
        o.selected = None;
        o.drag = None;
        if o.anim.is_none() {
            o.per_output.clear();
            o.strips.clear();
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
                    extents: border::style_for_overview(&self.hecate.config, w).style.extents(),
                })
                .collect();
            let slots = layout::grid(self.overview_area(output), &items, gap);
            let mut entries: Vec<Entry> = windows
                .into_iter()
                .zip(slots)
                .zip(from)
                .map(|((window, slot), from)| Entry { window, slot, from })
                .collect();
            entries.sort_by_key(|e| e.slot.index);
            per_output.insert(output.name(), entries);
            let thumbs = self.overview_thumbs(output);
            self.hecate.overview.strips.insert(output.name(), thumbs);
            let usable = Wm::usable_area(output, &self.hecate.config);
            self.hecate.overview.usable.insert(output.name(), usable);
        }
        self.hecate.overview.per_output = per_output;
        if let Some(d) = &self.hecate.overview.drag
            && !self.hecate.overview.contains(&d.window)
        {
            self.hecate.overview.drag = None;
        }
        let names: Vec<String> = outputs.iter().map(|o| o.name()).collect();
        self.hecate.overview.strips.retain(|n, _| names.contains(n));
        self.hecate.overview.usable.retain(|n, _| names.contains(n));

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

    /// An output's strip: one thumbnail per workspace it shows. None without the strip.
    fn overview_thumbs(&self, output: &Output) -> Vec<strip::Thumb> {
        let c = &self.hecate.config.overview;
        if !c.strip {
            return Vec::new();
        }
        let wm = &self.hecate.wm;
        let rows = Rows::new(Wm::usable_area(output, &self.hecate.config), c.gap, true);
        let workspaces = wm.workspaces_for(output);
        let size = Wm::output_geometry(output).size;
        let mut thumbs = strip::strip(rows.strip, size, workspaces.len(), c.gap.max(0), THUMB_EXTENTS);
        for (t, ws) in thumbs.iter_mut().zip(workspaces) {
            t.workspace = ws.id;
        }
        thumbs
    }

    /// Once per loop iteration: close for a lock or an exclusive-keyboard layer surface,
    /// follow windows coming and going and workspace switches, and drop a finished
    /// transition.
    pub(crate) fn refresh_overview(&mut self) {
        let now = self.hecate.wm.clock.now();
        if self.hecate.overview.open {
            if self.hecate.wm.is_locked() {
                self.close_overview_now();
            } else if self.keyboard_on_exclusive_layer() {
                self.close_overview_for_layer();
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
                    let ids: Vec<u64> = if self.hecate.config.overview.strip {
                        self.hecate.wm.workspaces_for(o).iter().map(|w| w.id).collect()
                    } else {
                        Vec::new()
                    };
                    let thumbs: Vec<u64> = self
                        .hecate
                        .overview
                        .strips
                        .get(&o.name())
                        .map(|t| t.iter().map(|t| t.workspace).collect())
                        .unwrap_or_default();
                    now.len() != shown.len() || now.iter().any(|w| !shown.contains(&w)) || ids != thumbs
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
