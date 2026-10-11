//! Stacking window management: outputs, workspaces, z-order.

pub mod geometry;
pub mod ops;
pub mod window;

use std::collections::HashMap;

use smithay::desktop::space::SpaceElement;
use smithay::desktop::{Window, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, Rectangle};
use smithay::wayland::session_lock::LockSurface;

use smithay::backend::renderer::ContextId;
use smithay::backend::renderer::element::Id;
use smithay::backend::renderer::gles::GlesTexture;

use crate::anim::{Animation, Clock};
use crate::config::{Animations, Config, MAX_WORKSPACES, WorkspaceKind, WorkspaceScope};
pub use window::{Maximized, WindowExt};

static NEXT_WORKSPACE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug)]
pub struct Workspace {
    pub id: u64,
    pub name: Option<String>,
    /// Owning output name in `per_output` scope.
    pub output: Option<String>,
    /// Bottom to top.
    pub windows: Vec<Window>,
    pub minimized: Vec<Window>,
}

impl Workspace {
    fn new(name: Option<String>, output: Option<String>) -> Self {
        Workspace {
            id: NEXT_WORKSPACE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            name,
            output,
            windows: Vec::new(),
            minimized: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty() && self.minimized.is_empty()
    }

    pub fn contains(&self, w: &Window) -> bool {
        self.windows.contains(w) || self.minimized.contains(w)
    }
}

#[derive(Debug, Default)]
pub enum LockState {
    #[default]
    Unlocked,
    Locked,
}

pub struct Wm {
    outputs: Vec<Output>,
    pub workspaces: Vec<Workspace>,
    pub scope: WorkspaceScope,
    pub kind: WorkspaceKind,
    count: u32,
    min: u32,
    names: Vec<String>,
    pub auto_back_and_forth: bool,
    /// Active workspace id per output name (per_output) or under "" (global).
    active: HashMap<String, u64>,
    previous: HashMap<String, u64>,
    /// Toplevels created but not mapped yet.
    pub pending: Vec<Window>,
    pub lock: LockState,
    lock_surfaces: HashMap<String, LockSurface>,
    pub show_desktop: bool,
    focus_counter: u64,
    pub focused_output: Option<Output>,
    /// Cleared when a workspace change should be announced to protocols/IPC.
    pub workspaces_dirty: bool,
    /// Windows by workspace index, waiting to be re-added after a scope change.
    pending_migration: Vec<Vec<Window>>,
    pub clock: Clock,
    pub anim: Animations,
    /// Workspace switch animations, by the same key as `active`.
    switches: HashMap<String, WorkspaceSwitch>,
    /// Snapshots of closed windows, fading out.
    pub closing: Vec<Closing>,
}

/// A workspace switch in progress: the old workspace slides out, the new one in.
#[derive(Debug, Clone)]
pub struct WorkspaceSwitch {
    pub anim: Animation,
    pub from: u64,
    /// +1 when the new workspace comes from the right (higher index), -1 from the left.
    pub direction: i32,
}

/// The last frame of a closed window, played out by its close animation.
#[derive(Debug)]
pub struct Closing {
    pub texture: GlesTexture,
    pub id: Id,
    pub context: ContextId<GlesTexture>,
    /// Area the snapshot covers (window plus border), global logical coordinates.
    pub rect: Rectangle<i32, Logical>,
    pub workspace: u64,
    pub anim: Animation,
}

impl Default for Wm {
    fn default() -> Self {
        Self::new()
    }
}

impl Wm {
    pub fn new() -> Self {
        Wm {
            outputs: Vec::new(),
            workspaces: Vec::new(),
            scope: WorkspaceScope::Global,
            kind: WorkspaceKind::Fixed,
            count: 9,
            min: 1,
            names: Vec::new(),
            auto_back_and_forth: false,
            active: HashMap::new(),
            previous: HashMap::new(),
            pending: Vec::new(),
            lock: LockState::Unlocked,
            lock_surfaces: HashMap::new(),
            show_desktop: false,
            focus_counter: 0,
            focused_output: None,
            workspaces_dirty: true,
            pending_migration: Vec::new(),
            clock: Clock::default(),
            anim: Animations::default(),
            switches: HashMap::new(),
            closing: Vec::new(),
        }
    }

    fn key(&self, output: &Output) -> String {
        match self.scope {
            WorkspaceScope::Global => String::new(),
            WorkspaceScope::PerOutput => output.name(),
        }
    }

    // ---------------------------------------------------------------- outputs

    pub fn outputs(&self) -> impl Iterator<Item = &Output> {
        self.outputs.iter()
    }

    pub fn output_geometry(output: &Output) -> Rectangle<i32, Logical> {
        geometry::output_geometry(output)
    }

    /// Output area minus layer-shell exclusive zones, in global coordinates.
    pub fn usable_area(output: &Output, config: &Config) -> Rectangle<i32, Logical> {
        let geo = Self::output_geometry(output);
        let map = layer_map_for_output(output);
        let mut zone = map.non_exclusive_zone();
        zone.loc += geo.loc;
        for m in &config.margins {
            if m.output.as_ref().is_none_or(|o| *o == output.name()) {
                zone.loc.x += m.left;
                zone.loc.y += m.top;
                zone.size.w -= m.left + m.right;
                zone.size.h -= m.top + m.bottom;
            }
        }
        zone
    }

    pub fn add_output(&mut self, output: Output, config: &Config) {
        if self.outputs.contains(&output) {
            return;
        }
        self.outputs.push(output.clone());
        if self.focused_output.is_none() {
            self.focused_output = Some(output.clone());
        }
        self.ensure_workspaces(config);
        self.workspaces_dirty = true;
    }

    pub fn remove_output(&mut self, output: &Output) {
        self.outputs.retain(|o| o != output);
        if self.focused_output.as_ref() == Some(output) {
            self.focused_output = self.outputs.first().cloned();
        }
        for ws in &mut self.workspaces {
            for w in ws.windows.iter().chain(ws.minimized.iter()) {
                let mut data = w.data();
                if let Some(pos) = data.entered_outputs.iter().position(|o| o == output) {
                    data.entered_outputs.remove(pos);
                    drop(data);
                    w.output_leave(output);
                }
            }
        }
        self.lock_surfaces.remove(&output.name());

        if self.scope == WorkspaceScope::PerOutput {
            // Move the removed output's windows to the first remaining output's active workspace.
            let name = output.name();
            let orphaned: Vec<Window> = self
                .workspaces
                .iter_mut()
                .filter(|ws| ws.output.as_deref() == Some(name.as_str()))
                .flat_map(|ws| {
                    let mut v = std::mem::take(&mut ws.windows);
                    v.append(&mut ws.minimized);
                    v
                })
                .collect();
            self.workspaces
                .retain(|ws| ws.output.as_deref() != Some(name.as_str()));
            self.active.remove(&name);
            if let Some(target) = self.outputs.first().cloned() {
                let target_geo = Self::output_geometry(&target);
                let ws_id = self.active_workspace_id(&target);
                for w in orphaned {
                    {
                        let mut d = w.data();
                        d.workspace = ws_id;
                        d.loc = target_geo.loc + Point::from((40, 40));
                    }
                    if let Some(ws) = self.workspace_mut(ws_id) {
                        ws.windows.push(w);
                    }
                }
            }
        }
        self.workspaces_dirty = true;
    }

    pub fn output_by_name(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.name() == name)
    }

    pub fn output_at(&self, point: Point<f64, Logical>) -> Option<&Output> {
        self.outputs
            .iter()
            .find(|o| Self::output_geometry(o).to_f64().contains(point))
    }

    /// The output containing most of the window.
    pub fn output_for_window(&self, window: &Window) -> Option<&Output> {
        let rect = self.window_rect(window);
        self.outputs
            .iter()
            .max_by_key(|o| {
                Self::output_geometry(o)
                    .intersection(rect)
                    .map(|r| r.size.w as i64 * r.size.h as i64)
                    .unwrap_or(0)
            })
            .filter(|o| Self::output_geometry(o).overlaps(rect))
            .or_else(|| {
                let ws = window.data().workspace;
                let owner = self.workspace(ws).and_then(|w| w.output.clone());
                owner.and_then(|n| self.output_by_name(&n))
            })
            .or(self.focused_output.as_ref())
    }

    /// Output in a direction from `from`, by comparing centres.
    pub fn output_in_direction(
        &self,
        from: &Output,
        dir: crate::config::action::Direction,
    ) -> Option<Output> {
        use crate::config::action::Direction;
        let c = geometry::center(Self::output_geometry(from));
        self.outputs
            .iter()
            .filter(|o| *o != from)
            .filter_map(|o| {
                let oc = geometry::center(Self::output_geometry(o));
                let (dx, dy) = (oc.x - c.x, oc.y - c.y);
                let ok = match dir {
                    Direction::Left => dx < 0 && dx.abs() >= dy.abs(),
                    Direction::Right => dx > 0 && dx.abs() >= dy.abs(),
                    Direction::Up => dy < 0 && dy.abs() >= dx.abs(),
                    Direction::Down => dy > 0 && dy.abs() >= dx.abs(),
                };
                ok.then_some((o, dx as i64 * dx as i64 + dy as i64 * dy as i64))
            })
            .min_by_key(|(_, d)| *d)
            .map(|(o, _)| o.clone())
    }

    /// Output by reference string: direction, next/prev or name.
    pub fn resolve_output(&self, from: &Output, r: &str) -> Option<Output> {
        use crate::config::action::Direction;
        let idx = self.outputs.iter().position(|o| o == from).unwrap_or(0);
        let n = self.outputs.len();
        match r {
            "left" => self.output_in_direction(from, Direction::Left),
            "right" => self.output_in_direction(from, Direction::Right),
            "up" => self.output_in_direction(from, Direction::Up),
            "down" => self.output_in_direction(from, Direction::Down),
            "next" if n > 0 => self.outputs.get((idx + 1) % n).cloned(),
            "prev" | "previous" if n > 0 => self.outputs.get((idx + n - 1) % n).cloned(),
            name => self.output_by_name(name).cloned(),
        }
    }

    // ------------------------------------------------------------- workspaces

    /// Apply workspace settings from the config and create/remove workspaces to match.
    pub fn ensure_workspaces(&mut self, config: &Config) {
        let ws = &config.workspaces;
        let scope_changed = self.scope != ws.scope && !self.workspaces.is_empty();
        self.scope = ws.scope;
        self.kind = ws.kind;
        self.count = ws.count.clamp(1, MAX_WORKSPACES);
        self.min = ws.min.clamp(1, MAX_WORKSPACES);
        self.names = ws.names.clone();
        self.auto_back_and_forth = ws.auto_back_and_forth;
        self.anim = config.animations.clone();

        if scope_changed {
            self.migrate_scope();
        }

        let groups: Vec<Option<String>> = match self.scope {
            WorkspaceScope::Global => vec![None],
            WorkspaceScope::PerOutput => self.outputs.iter().map(|o| Some(o.name())).collect(),
        };
        for group in groups {
            let want = match self.kind {
                WorkspaceKind::Fixed => self.count as usize,
                WorkspaceKind::Auto => self.min as usize,
            };
            let have = self.group_ids(group.as_deref()).len();
            for i in have..want {
                let name = self.names.get(i).cloned();
                self.workspaces.push(Workspace::new(name, group.clone()));
            }
            if self.kind == WorkspaceKind::Fixed {
                self.trim_group(group.as_deref(), want);
            }
            let key = group.clone().unwrap_or_default();
            let ids = self.group_ids(group.as_deref());
            if !self.active.get(&key).is_some_and(|id| ids.contains(id))
                && let Some(first) = ids.first()
            {
                self.active.insert(key, *first);
            }
            if self.kind == WorkspaceKind::Auto {
                self.auto_fixup(group.as_deref());
            }
        }
        // Apply names to fixed workspaces in order.
        if self.kind == WorkspaceKind::Fixed {
            let groups: Vec<Option<String>> = self.workspaces.iter().map(|w| w.output.clone()).collect();
            let mut seen: HashMap<Option<String>, usize> = HashMap::new();
            for (i, g) in groups.into_iter().enumerate() {
                let idx = seen.entry(g).or_default();
                self.workspaces[i].name = self.names.get(*idx).cloned();
                *idx += 1;
            }
        }
        self.workspaces_dirty = true;
    }

    /// Remove workspaces beyond `want` in a group, moving their windows to the last kept one.
    fn trim_group(&mut self, group: Option<&str>, want: usize) {
        let ids = self.group_ids(group);
        if ids.len() <= want {
            return;
        }
        let keep_last = ids[want - 1];
        let mut moved = Vec::new();
        for id in &ids[want..] {
            if let Some(ws) = self.workspace_mut(*id) {
                moved.append(&mut ws.windows);
                moved.append(&mut ws.minimized);
            }
        }
        let removed: Vec<u64> = ids[want..].to_vec();
        self.workspaces.retain(|w| !removed.contains(&w.id));
        for w in moved {
            w.data().workspace = keep_last;
            if let Some(ws) = self.workspace_mut(keep_last) {
                ws.windows.push(w);
            }
        }
        for v in self.active.values_mut() {
            if removed.contains(v) {
                *v = keep_last;
            }
        }
    }

    /// Switching global <-> per_output: rebuild workspaces, keeping windows by index.
    fn migrate_scope(&mut self) {
        let old = std::mem::take(&mut self.workspaces);
        self.active.clear();
        self.previous.clear();
        let mut by_index: Vec<Vec<Window>> = Vec::new();
        let mut counters: HashMap<Option<String>, usize> = HashMap::new();
        for mut ws in old {
            let idx = counters.entry(ws.output.clone()).or_default();
            if by_index.len() <= *idx {
                by_index.resize_with(*idx + 1, Vec::new);
            }
            by_index[*idx].append(&mut ws.windows);
            by_index[*idx].append(&mut ws.minimized);
            *idx += 1;
        }
        // New workspaces get created by ensure_workspaces; stash windows to re-add.
        self.pending_migration = by_index;
    }

    fn group_ids(&self, group: Option<&str>) -> Vec<u64> {
        self.workspaces
            .iter()
            .filter(|w| w.output.as_deref() == group)
            .map(|w| w.id)
            .collect()
    }

    fn group_of_output(&self, output: &Output) -> Option<String> {
        match self.scope {
            WorkspaceScope::Global => None,
            WorkspaceScope::PerOutput => Some(output.name()),
        }
    }

    /// Workspaces visible to an output, in order.
    pub fn workspaces_for(&self, output: &Output) -> Vec<&Workspace> {
        let g = self.group_of_output(output);
        self.workspaces.iter().filter(|w| w.output == g).collect()
    }

    pub fn workspace(&self, id: u64) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn workspace_mut(&mut self, id: u64) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    pub fn active_workspace_id(&self, output: &Output) -> u64 {
        self.active.get(&self.key(output)).copied().unwrap_or(0)
    }

    pub fn active_workspace(&self, output: &Output) -> Option<&Workspace> {
        self.workspace(self.active_workspace_id(output))
    }

    pub fn is_workspace_active(&self, id: u64) -> bool {
        self.active.values().any(|v| *v == id)
    }

    /// 1-based index of a workspace within its group.
    pub fn workspace_index(&self, id: u64) -> Option<usize> {
        let ws = self.workspace(id)?;
        let g = ws.output.clone();
        self.workspaces
            .iter()
            .filter(|w| w.output == g)
            .position(|w| w.id == id)
            .map(|i| i + 1)
    }

    pub fn workspace_label(&self, id: u64) -> String {
        let ws = self.workspace(id);
        ws.and_then(|w| w.name.clone())
            .unwrap_or_else(|| self.workspace_index(id).unwrap_or(0).to_string())
    }

    /// Resolve a workspace reference for an output; may create one in auto mode.
    pub fn resolve_workspace(
        &mut self,
        output: &Output,
        r: &crate::config::action::WorkspaceRef,
    ) -> Option<u64> {
        use crate::config::action::WorkspaceRef;
        let group = self.group_of_output(output);
        let ids = self.group_ids(group.as_deref());
        let cur = self.active_workspace_id(output);
        let cur_idx = ids.iter().position(|i| *i == cur).unwrap_or(0);
        match r {
            WorkspaceRef::Named(n) if n == "next" => {
                if cur_idx + 1 < ids.len() {
                    Some(ids[cur_idx + 1])
                } else if self.kind == WorkspaceKind::Fixed {
                    Some(ids[0])
                } else {
                    None
                }
            }
            WorkspaceRef::Named(n) if n == "prev" || n == "previous" => {
                if cur_idx > 0 {
                    Some(ids[cur_idx - 1])
                } else if self.kind == WorkspaceKind::Fixed {
                    ids.last().copied()
                } else {
                    None
                }
            }
            WorkspaceRef::Named(n) => self
                .workspaces
                .iter()
                .find(|w| w.output == group && w.name.as_deref() == Some(n.as_str()))
                .map(|w| w.id)
                .or_else(|| n.parse::<u32>().ok().and_then(|i| self.nth_workspace(output, i))),
            WorkspaceRef::Index(i) => self.nth_workspace(output, *i),
        }
    }

    fn nth_workspace(&mut self, output: &Output, i: u32) -> Option<u64> {
        if i == 0 || i > MAX_WORKSPACES {
            return None;
        }
        let group = self.group_of_output(output);
        let ids = self.group_ids(group.as_deref());
        if let Some(id) = ids.get(i as usize - 1) {
            return Some(*id);
        }
        if self.kind == WorkspaceKind::Auto {
            // Create up to the requested index.
            for _ in ids.len()..i as usize {
                self.workspaces.push(Workspace::new(None, group.clone()));
            }
            self.workspaces_dirty = true;
            return self.group_ids(group.as_deref()).get(i as usize - 1).copied();
        }
        None
    }

    /// Activate a workspace on an output. Returns false if nothing changed.
    pub fn activate_workspace(&mut self, output: &Output, id: u64) -> bool {
        let key = self.key(output);
        let cur = self.active.get(&key).copied();
        let id = if cur == Some(id) && self.auto_back_and_forth {
            match self.previous.get(&key) {
                Some(prev) if self.workspace(*prev).is_some() => *prev,
                _ => return false,
            }
        } else {
            id
        };
        if cur == Some(id) {
            return false;
        }
        // Omnipresent windows follow.
        if let Some(cur) = cur {
            self.start_switch(&key, cur, id);
            let sticky: Vec<Window> = self
                .workspace(cur)
                .map(|w| {
                    w.windows
                        .iter()
                        .filter(|w| w.data().omnipresent)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            for w in sticky {
                self.move_window_to_workspace_raw(&w, id);
            }
            self.previous.insert(key.clone(), cur);
        }
        self.active.insert(key, id);
        self.show_desktop = false;
        let group = self.workspace(id).and_then(|w| w.output.clone());
        if self.kind == WorkspaceKind::Auto {
            self.auto_fixup(group.as_deref());
        }
        self.workspaces_dirty = true;
        true
    }

    /// Auto mode: exactly one empty workspace at the end; drop inactive empty ones beyond `min`.
    fn auto_fixup(&mut self, group: Option<&str>) {
        let active: Vec<u64> = self.active.values().copied().collect();
        loop {
            let ids = self.group_ids(group);
            let removable = ids.iter().enumerate().find(|(i, id)| {
                *i + 1 < ids.len()
                    && ids.len() > self.min as usize
                    && !active.contains(id)
                    && self.workspace(**id).is_some_and(|w| w.is_empty())
            });
            match removable {
                Some((_, id)) => {
                    let id = *id;
                    self.workspaces.retain(|w| w.id != id);
                }
                None => break,
            }
        }
        let ids = self.group_ids(group);
        let last_empty = ids
            .last()
            .and_then(|id| self.workspace(*id))
            .is_some_and(|w| w.is_empty());
        if !last_empty && ids.len() < MAX_WORKSPACES as usize {
            self.workspaces
                .push(Workspace::new(None, group.map(String::from)));
        }
        // Collapse trailing empties down to one (never removing an active one).
        loop {
            let ids = self.group_ids(group);
            if ids.len() < 2 || ids.len() <= self.min as usize {
                break;
            }
            let last = ids[ids.len() - 1];
            let before = ids[ids.len() - 2];
            let both_empty = self.workspace(last).is_some_and(|w| w.is_empty())
                && self.workspace(before).is_some_and(|w| w.is_empty());
            if both_empty && !active.contains(&last) {
                self.workspaces.retain(|w| w.id != last);
            } else {
                break;
            }
        }
    }

    // ---------------------------------------------------------------- windows

    pub fn all_windows(&self) -> impl Iterator<Item = &Window> {
        self.workspaces
            .iter()
            .flat_map(|w| w.windows.iter().chain(w.minimized.iter()))
            .chain(self.pending.iter())
    }

    /// Mapped, visible windows on the active workspace(s) of an output, bottom to top.
    pub fn windows_on_output(&self, output: &Output) -> Vec<Window> {
        if self.show_desktop {
            return Vec::new();
        }
        let geo = Self::output_geometry(output);
        self.active_workspace(output)
            .map(|ws| {
                ws.windows
                    .iter()
                    .filter(|w| self.window_rect(w).overlaps(geo))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Windows visible anywhere, bottom to top (per active workspace).
    pub fn visible_windows(&self) -> Vec<Window> {
        if self.show_desktop {
            return Vec::new();
        }
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for o in &self.outputs {
            let id = self.active_workspace_id(o);
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if let Some(ws) = self.workspace(id) {
                out.extend(ws.windows.iter().cloned());
            }
        }
        out
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.all_windows()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned()
    }

    pub fn window_by_id(&self, id: u64) -> Option<Window> {
        self.all_windows().find(|w| w.wid() == id).cloned()
    }

    pub fn workspace_of(&self, window: &Window) -> Option<&Workspace> {
        self.workspaces.iter().find(|ws| ws.contains(window))
    }

    pub fn window_rect(&self, window: &Window) -> Rectangle<i32, Logical> {
        let loc = window.data().loc;
        Rectangle::new(loc, window.geometry().size)
    }

    /// Where to render the window's surface tree (geometry offset applied).
    pub fn render_location(window: &Window) -> Point<i32, Logical> {
        window.data().loc - window.geometry().loc
    }

    /// Insert a window into a workspace at the right z-band, on top of its band.
    pub fn insert_window(&mut self, window: Window, ws_id: u64) {
        window.data().workspace = ws_id;
        if let Some(ws) = self.workspace_mut(ws_id) {
            ws.windows.retain(|w| w != &window);
            ws.minimized.retain(|w| w != &window);
            ws.windows.push(window);
            sort_bands(&mut ws.windows);
        }
        self.workspaces_dirty = true;
        if self.kind == WorkspaceKind::Auto {
            let group = self.workspace(ws_id).and_then(|w| w.output.clone());
            self.auto_fixup(group.as_deref());
        }
    }

    pub fn remove_window(&mut self, window: &Window) {
        self.pending.retain(|w| w != window);
        let mut group = None;
        for ws in &mut self.workspaces {
            let before = ws.windows.len() + ws.minimized.len();
            ws.windows.retain(|w| w != window);
            ws.minimized.retain(|w| w != window);
            if ws.windows.len() + ws.minimized.len() != before {
                group = Some(ws.output.clone());
            }
        }
        let outputs: Vec<Output> = std::mem::take(&mut window.data().entered_outputs);
        for o in outputs {
            window.output_leave(&o);
        }
        if let Some(g) = group {
            self.workspaces_dirty = true;
            if self.kind == WorkspaceKind::Auto {
                self.auto_fixup(g.as_deref());
            }
        }
    }

    pub fn raise(&mut self, window: &Window) {
        for ws in &mut self.workspaces {
            if let Some(pos) = ws.windows.iter().position(|w| w == window) {
                let w = ws.windows.remove(pos);
                ws.windows.push(w);
                sort_bands(&mut ws.windows);
                return;
            }
        }
    }

    pub fn lower(&mut self, window: &Window) {
        for ws in &mut self.workspaces {
            if let Some(pos) = ws.windows.iter().position(|w| w == window) {
                let w = ws.windows.remove(pos);
                ws.windows.insert(0, w);
                sort_bands(&mut ws.windows);
                return;
            }
        }
    }

    /// Re-sort z-bands after a window's always-on-top/bottom changed.
    pub fn restack(&mut self) {
        for ws in &mut self.workspaces {
            sort_bands(&mut ws.windows);
        }
    }

    fn move_window_to_workspace_raw(&mut self, window: &Window, ws_id: u64) {
        let minimized = window.data().minimized;
        for ws in &mut self.workspaces {
            ws.windows.retain(|w| w != window);
            ws.minimized.retain(|w| w != window);
        }
        window.data().workspace = ws_id;
        if let Some(ws) = self.workspace_mut(ws_id) {
            if minimized {
                ws.minimized.push(window.clone());
            } else {
                ws.windows.push(window.clone());
                sort_bands(&mut ws.windows);
            }
        }
    }

    /// Move a window to another workspace. In per_output scope, the window also moves to that
    /// workspace's output (keeping its relative position).
    pub fn move_window_to_workspace(&mut self, window: &Window, ws_id: u64) {
        let target_output = self
            .workspace(ws_id)
            .and_then(|w| w.output.clone())
            .and_then(|n| self.output_by_name(&n).cloned());
        let current_output = self.output_for_window(window).cloned();
        self.move_window_to_workspace_raw(window, ws_id);
        if let (Some(from), Some(to)) = (current_output, target_output)
            && from != to
        {
            self.translate_between_outputs(window, &from, &to);
        }
        self.workspaces_dirty = true;
        let groups: Vec<Option<String>> = self.workspaces.iter().map(|w| w.output.clone()).collect();
        if self.kind == WorkspaceKind::Auto {
            let mut uniq = groups;
            uniq.dedup();
            for g in uniq {
                self.auto_fixup(g.as_deref());
            }
        }
    }

    /// Keep the window at the same relative spot when moving between outputs.
    pub fn translate_between_outputs(&mut self, window: &Window, from: &Output, to: &Output) {
        let fg = Self::output_geometry(from);
        let tg = Self::output_geometry(to);
        let size = window.geometry().size;
        let mut d = window.data();
        let rel_x = (d.loc.x - fg.loc.x) as f64 / fg.size.w.max(1) as f64;
        let rel_y = (d.loc.y - fg.loc.y) as f64 / fg.size.h.max(1) as f64;
        let mut x = tg.loc.x + (rel_x * tg.size.w as f64) as i32;
        let mut y = tg.loc.y + (rel_y * tg.size.h as f64) as i32;
        x = x.clamp(tg.loc.x, (tg.loc.x + tg.size.w - size.w).max(tg.loc.x));
        y = y.clamp(tg.loc.y, (tg.loc.y + tg.size.h - size.h).max(tg.loc.y));
        d.loc = (x, y).into();
        if let Some(r) = d.restore.as_mut() {
            r.loc = (x, y).into();
        }
    }

    /// Topmost window and surface under a point on visible workspaces.
    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(Window, WlSurface, Point<f64, Logical>)> {
        let output = self.output_at(point)?;
        let windows = self.windows_on_output(output);
        for w in windows.iter().rev() {
            let render_loc = Self::render_location(w).to_f64();
            if let Some((surface, surface_loc)) = w.surface_under(point - render_loc, WindowSurfaceType::ALL)
            {
                return Some((w.clone(), surface, surface_loc.to_f64() + render_loc));
            }
        }
        None
    }

    /// Topmost window whose outer rect (including `extra` border) contains the point.
    pub fn window_at(&self, point: Point<f64, Logical>, extra: i32) -> Option<Window> {
        let output = self.output_at(point)?;
        self.windows_on_output(output).into_iter().rev().find(|w| {
            let mut r = self.window_rect(w);
            r.loc -= (extra, extra).into();
            r.size += (extra * 2, extra * 2).into();
            r.to_f64().contains(point)
        })
    }

    pub fn next_focus_serial(&mut self) -> u64 {
        self.focus_counter += 1;
        self.focus_counter
    }

    /// Visible windows ordered by most recently focused first.
    pub fn mru(&self) -> Vec<Window> {
        let mut v = self.visible_windows();
        v.sort_by_key(|w| std::cmp::Reverse(w.data().focus_serial));
        v
    }

    pub fn topmost_on_output(&self, output: &Output) -> Option<Window> {
        self.windows_on_output(output).last().cloned()
    }

    /// Per-frame housekeeping: output enter/leave, dead windows.
    pub fn refresh(&mut self) {
        let dead: Vec<Window> = self.all_windows().filter(|w| !w.alive()).cloned().collect();
        for w in dead {
            self.remove_window(&w);
        }
        self.lock_surfaces.retain(|_, s| s.alive());

        let visible = self.visible_windows();
        for w in self.all_windows().cloned().collect::<Vec<_>>() {
            let rect = {
                let r = self.window_rect(&w);
                // Use the bounding box so popups/subsurfaces count too.
                let bbox = w.bbox();
                Rectangle::new(r.loc - w.geometry().loc + bbox.loc, bbox.size)
            };
            let is_visible = visible.contains(&w);
            for o in &self.outputs {
                let geo = Self::output_geometry(o);
                let overlap = if is_visible { geo.intersection(rect) } else { None };
                let entered = w.data().entered_outputs.contains(o);
                match overlap {
                    Some(mut r) => {
                        r.loc -= w.data().loc - w.geometry().loc;
                        w.output_enter(o, r);
                        if !entered {
                            w.data().entered_outputs.push(o.clone());
                        }
                    }
                    None if entered => {
                        w.output_leave(o);
                        w.data().entered_outputs.retain(|x| x != o);
                    }
                    None => {}
                }
            }
            w.refresh();
        }

        if !self.pending_migration.is_empty() {
            let windows = std::mem::take(&mut self.pending_migration);
            for (i, list) in windows.into_iter().enumerate() {
                let target = self.outputs.first().cloned().and_then(|o| {
                    let g = self.group_of_output(&o);
                    let ids = self.group_ids(g.as_deref());
                    ids.get(i).or(ids.last()).copied()
                });
                if let Some(t) = target {
                    for w in list {
                        self.move_window_to_workspace_raw(&w, t);
                    }
                }
            }
            self.workspaces_dirty = true;
        }
    }

    fn start_switch(&mut self, key: &str, from: u64, to: u64) {
        // Windows on the workspace being hidden start from scratch when it's shown again.
        if let Some(ws) = self.workspace(from) {
            for w in &ws.windows {
                w.data().anims.last_target = None;
            }
        }
        let (Some(fi), Some(ti)) = (self.workspace_index(from), self.workspace_index(to)) else {
            return;
        };
        match Animation::start(&self.clock, &self.anim, &self.anim.workspace_switch) {
            Some(anim) => {
                let direction = if ti >= fi { 1 } else { -1 };
                self.switches.insert(
                    key.to_string(),
                    WorkspaceSwitch {
                        anim,
                        from,
                        direction,
                    },
                );
            }
            None => {
                self.switches.remove(key);
            }
        }
    }

    /// The workspace switch animating on an output, if any.
    pub fn switch_for(&self, output: &Output) -> Option<&WorkspaceSwitch> {
        self.switches.get(&self.key(output))
    }

    /// Drop finished animations. Returns whether anything is still animating on the output.
    pub fn advance_animations(&mut self, output: &Output) -> bool {
        let now = self.clock.now();
        let mut running = false;
        let key = self.key(output);
        if let Some(s) = self.switches.get(&key) {
            if s.anim.is_done(now) || self.workspace(s.from).is_none() {
                self.switches.remove(&key);
            } else {
                running = true;
            }
        }
        self.closing.retain(|c| !c.anim.is_done(now));
        let geo = Self::output_geometry(output);
        running |= self.closing.iter().any(|c| c.rect.overlaps(geo));
        for w in self.windows_on_output(output) {
            let mut d = w.data();
            let a = &mut d.anims;
            if a.open.as_ref().is_some_and(|x| x.is_done(now)) {
                a.open = None;
            }
            if a.moving.as_ref().is_some_and(|(x, _)| x.is_done(now)) {
                a.moving = None;
            }
            if a.border.as_ref().is_some_and(|(x, _)| x.is_done(now)) {
                a.border = None;
            }
            running |= a.open.is_some() || a.moving.is_some() || a.border.is_some();
        }
        running
    }

    // -------------------------------------------------------------- lock

    pub fn lock_surface(&self, output: &Output) -> Option<&LockSurface> {
        self.lock_surfaces.get(&output.name())
    }

    pub fn set_lock_surface(&mut self, output: &Output, surface: LockSurface) {
        self.lock_surfaces.insert(output.name(), surface);
    }

    pub fn is_locked(&self) -> bool {
        matches!(self.lock, LockState::Locked)
    }

    pub fn unlock(&mut self) {
        self.lock = LockState::Unlocked;
        self.lock_surfaces.clear();
    }
}

/// Keep always-on-bottom windows at the start and always-on-top at the end, stable otherwise.
fn sort_bands(windows: &mut [Window]) {
    windows.sort_by_key(|w| {
        let d = w.data();
        if d.fullscreen {
            3
        } else if d.always_on_top {
            2
        } else if d.always_on_bottom {
            0
        } else {
            1
        }
    });
}
