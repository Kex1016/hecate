//! Window management operations that need the whole compositor state.

use smithay::desktop::Window;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Size};

use super::geometry::{self, centered, clamp_into, snap_rect};
use super::window::{Maximized, ResizeEdge, ResolvedRules, Snap, clamp_size};
use super::{WindowExt, Wm};
use crate::config::action::Edge;
use crate::config::{PlacementPolicy, WindowRule};
use crate::state::State;

/// Does a window rule's match block apply?
pub fn rule_matches(rule: &WindowRule, app_id: &str, title: &str, sandbox: Option<&str>) -> bool {
    let m = &rule.matches;
    let glob = |pat: &Option<String>, value: &str| {
        pat.as_ref()
            .is_none_or(|p| glob::Pattern::new(p).is_ok_and(|g| g.matches(value)))
    };
    glob(&m.app_id, app_id)
        && glob(&m.title, title)
        && m.sandbox_engine
            .as_ref()
            .is_none_or(|s| sandbox == Some(s.as_str()))
}

impl State {
    /// Compute rule results; on first map also apply initial-state properties.
    pub fn apply_window_rules(&mut self, window: &Window, initial: bool) {
        let app_id = window.app_id();
        let title = window.title();
        let sandbox = self.sandbox_engine_of(window);
        let mut resolved = ResolvedRules::default();
        let mut initial_props: Vec<WindowRule> = Vec::new();
        for (i, rule) in self.hecate.config.window_rules.iter().enumerate() {
            if !rule_matches(rule, &app_id, &title, sandbox.as_deref()) {
                continue;
            }
            resolved.matched.push(i);
            if let Some(b) = &rule.border {
                let merged = resolved.border.get_or_insert_with(Default::default);
                if b.width.is_some() {
                    merged.width = b.width;
                }
                if b.radius.is_some() {
                    merged.radius = b.radius;
                }
                if b.color.is_some() {
                    merged.color = b.color;
                }
                if b.clip_content.is_some() {
                    merged.clip_content = b.clip_content;
                }
            }
            if let Some(v) = rule.decorations {
                resolved.decorations = Some(v);
            }
            macro_rules! flag {
                ($f:ident) => {
                    if let Some(v) = rule.$f {
                        resolved.$f = v;
                    }
                };
            }
            flag!(tearing);
            flag!(skip_taskbar);
            flag!(ignore_focus_requests);
            flag!(fixed_position);
            flag!(no_animations);
            if rule.opacity.is_some() {
                resolved.opacity = rule.opacity;
            }
            initial_props.push(rule.clone());
        }
        let changed_deco = window.data().rules.decorations != resolved.decorations;
        window.data().rules = resolved;

        if initial {
            let mut d = window.data();
            for rule in &initial_props {
                if let Some(v) = rule.always_on_top {
                    d.always_on_top = v;
                }
                if let Some(v) = rule.always_on_bottom {
                    d.always_on_bottom = v;
                }
                if let Some(v) = rule.omnipresent {
                    d.omnipresent = v;
                }
            }
        }
        if changed_deco {
            let toplevel = window.toplevel_surface().clone();
            let mode = self.decoration_mode_for(&toplevel);
            toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
            if toplevel.is_initial_configure_sent() {
                toplevel.send_pending_configure();
            }
        }
    }

    fn sandbox_engine_of(&self, window: &Window) -> Option<String> {
        let client = window.toplevel()?.wl_surface().client()?;
        let data = client.get_data::<crate::state::ClientState>()?;
        data.security_context
            .as_ref()
            .and_then(|c| c.sandbox_engine.clone())
    }

    fn rules_for(&self, window: &Window) -> Vec<WindowRule> {
        let idx = window.data().rules.matched.clone();
        idx.iter()
            .filter_map(|i| self.hecate.config.window_rules.get(*i).cloned())
            .collect()
    }

    /// Set states (maximized/fullscreen) and bounds before the initial configure.
    pub fn prepare_initial_configure(&mut self, window: &Window) {
        let output = self.target_output_for_new(window);
        let toplevel = window.toplevel_surface().clone();
        let rules = self.rules_for(window);
        let maximized = rules.iter().rev().find_map(|r| r.maximized).unwrap_or(false)
            || toplevel.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized));
        let fullscreen = rules.iter().rev().find_map(|r| r.fullscreen).unwrap_or(false)
            || toplevel.with_pending_state(|s| s.states.contains(xdg_toplevel::State::Fullscreen));
        let size = rules.iter().rev().find_map(|r| r.size);
        let mode = self.decoration_mode_for(&toplevel);

        if let Some(output) = &output {
            let area = Wm::usable_area(output, &self.hecate.config);
            let full = Wm::output_geometry(output);
            toplevel.with_pending_state(|s| {
                s.bounds = Some(area.size);
                s.decoration_mode = Some(mode);
                if fullscreen {
                    s.states.set(xdg_toplevel::State::Fullscreen);
                    s.size = Some(full.size);
                } else if maximized {
                    s.states.set(xdg_toplevel::State::Maximized);
                    s.size = Some(self.maximized_rect(output, Maximized::Both, None).size);
                } else if let Some([w, h]) = size {
                    s.size = Some((w, h).into());
                }
            });
            let scale = output.current_scale();
            let transform = output.current_transform();
            smithay::wayland::compositor::with_states(toplevel.wl_surface(), |states| {
                crate::utils::send_scale_transform(toplevel.wl_surface(), states, scale, transform);
            });
        }
        let mut d = window.data();
        if fullscreen {
            d.fullscreen = true;
        } else if maximized {
            d.maximized = Maximized::Both;
        }
    }

    fn target_output_for_new(&self, window: &Window) -> Option<Output> {
        let rules = self.rules_for(window);
        if let Some(name) = rules.iter().rev().find_map(|r| r.output.clone())
            && let Some(o) = self.hecate.wm.output_by_name(&name)
        {
            return Some(o.clone());
        }
        // A parent window's output, else the focused output.
        if let Some(parent) = window.toplevel().and_then(|t| t.parent())
            && let Some(pw) = self.hecate.window_for_surface(&parent)
            && let Some(o) = self.hecate.wm.output_for_window(&pw)
        {
            return Some(o.clone());
        }
        self.hecate
            .wm
            .focused_output
            .clone()
            .or_else(|| self.hecate.wm.outputs().next().cloned())
    }

    /// First buffer committed: place the window and show it.
    pub fn map_new_window(&mut self, window: Window) {
        let Some(output) = self.target_output_for_new(&window) else {
            self.hecate.wm.pending.push(window);
            return;
        };
        let rules = self.rules_for(&window);
        let ws_id = rules
            .iter()
            .rev()
            .find_map(|r| r.workspace.clone())
            .and_then(|r| self.hecate.wm.resolve_workspace(&output, &r))
            .unwrap_or_else(|| self.hecate.wm.active_workspace_id(&output));

        let size = window.geometry().size;
        let area = Wm::usable_area(&output, &self.hecate.config);
        let (fullscreen, maximized) = {
            let d = window.data();
            (d.fullscreen, d.maximized)
        };
        let loc = if fullscreen {
            Wm::output_geometry(&output).loc
        } else if maximized != Maximized::No {
            self.maximized_rect(&output, maximized, None).loc
        } else if let Some([x, y]) = rules.iter().rev().find_map(|r| r.position) {
            let og = Wm::output_geometry(&output);
            (og.loc.x + x, og.loc.y + y).into()
        } else if let Some(parent) = window.toplevel().and_then(|t| t.parent())
            && let Some(pw) = self.hecate.window_for_surface(&parent)
        {
            // Dialogs: centre over the parent.
            let pr = self.hecate.wm.window_rect(&pw);
            clamp_into(area, Rectangle::new(centered(pr, size), size))
        } else {
            self.place(&output, size, ws_id)
        };
        window.data().loc = loc;

        self.hecate.wm.insert_window(window.clone(), ws_id);
        if fullscreen {
            self.hecate.wm.restack();
        }
        let visible = self.hecate.wm.is_workspace_active(ws_id);
        if visible {
            self.focus_window(&window);
        }
        self.on_window_opened(&window);
        self.refresh_idle_inhibit();
        self.hecate.queue_redraw_all();
    }

    fn place(
        &self,
        output: &Output,
        size: Size<i32, Logical>,
        ws_id: u64,
    ) -> smithay::utils::Point<i32, Logical> {
        let area = Wm::usable_area(output, &self.hecate.config);
        match self.hecate.config.placement.policy {
            PlacementPolicy::Center => centered(area, size),
            PlacementPolicy::Cursor => {
                let p = self.hecate.pointer_location.to_i32_round::<i32>();
                clamp_into(
                    area,
                    Rectangle::new((p.x - size.w / 2, p.y - size.h / 2).into(), size),
                )
            }
            PlacementPolicy::Cascade => {
                let [dx, dy] = self.hecate.config.placement.cascade_offset;
                let top = self
                    .hecate
                    .wm
                    .workspace(ws_id)
                    .and_then(|ws| ws.windows.last().cloned());
                match top {
                    Some(t) => {
                        let r = self.hecate.wm.window_rect(&t);
                        let mut p = r.loc + Point::from((dx, dy));
                        if p.x + size.w > area.loc.x + area.size.w || p.y + size.h > area.loc.y + area.size.h
                        {
                            p = area.loc;
                        }
                        p
                    }
                    None => centered(area, size),
                }
            }
            PlacementPolicy::Automatic => {
                let existing: Vec<_> = self
                    .hecate
                    .wm
                    .workspace(ws_id)
                    .map(|ws| ws.windows.iter().map(|w| self.hecate.wm.window_rect(w)).collect())
                    .unwrap_or_default();
                geometry::place_automatic(area, size, &existing)
            }
        }
    }

    /// Commit on a mapped window: keep resize anchors in place.
    pub fn on_window_commit(&mut self, window: &Window, old_size: Size<i32, Logical>) {
        let new_size = window.geometry().size;
        let mut d = window.data();
        if new_size != old_size
            && let (Some(edge), Some(initial)) = (d.resize_edge, d.resize_initial)
        {
            if edge.left() {
                d.loc.x = initial.loc.x + initial.size.w - new_size.w;
            }
            if edge.top() {
                d.loc.y = initial.loc.y + initial.size.h - new_size.h;
            }
        }
        // The grab ended and the client caught up: drop the anchors.
        let pending = window.toplevel_surface().with_pending_state(|s| s.size);
        if !d.resizing && (pending.is_none() || pending == Some(new_size)) {
            d.resize_edge = None;
            d.resize_initial = None;
        }
    }

    pub fn on_window_opened(&mut self, _window: &Window) {
        self.hecate.wm.workspaces_dirty = true;
    }

    pub fn on_window_closed(&mut self, _window: &Window) {
        self.hecate.wm.workspaces_dirty = true;
        self.refresh_idle_inhibit();
    }

    pub fn on_window_changed(&mut self, _window: &Window) {}

    // ---------------------------------------------------------------- focus

    pub fn focused_window(&self) -> Option<Window> {
        let keyboard = self.hecate.seat.get_keyboard()?;
        let focus = keyboard.current_focus()?;
        self.hecate
            .window_for_surface(&crate::utils::root_surface(&focus))
    }

    /// Focus a window (raising per config). Doesn't switch workspaces.
    pub fn focus_window(&mut self, window: &Window) {
        if self.hecate.wm.is_locked() {
            return;
        }
        // An exclusive layer surface on top keeps the keyboard.
        if let Some(layer) = self.exclusive_layer_focus() {
            let keyboard = self.hecate.seat.get_keyboard().unwrap();
            keyboard.set_focus(self, Some(layer), SERIAL_COUNTER.next_serial());
            return;
        }
        let surface = window.toplevel_surface().wl_surface().clone();
        let previous = self.focused_window();
        if let Some(prev) = &previous
            && prev != window
        {
            prev.set_activated_state(false);
        }
        window.set_activated_state(true);
        {
            let serial = self.hecate.wm.next_focus_serial();
            let mut d = window.data();
            d.focus_serial = serial;
            d.urgent = false;
        }
        if self.hecate.config.focus.raise_on_focus {
            self.hecate.wm.raise(window);
        }
        if let Some(o) = self.hecate.wm.output_for_window(window).cloned() {
            self.hecate.wm.focused_output = Some(o);
        }
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(surface), SERIAL_COUNTER.next_serial());
        self.hecate.queue_redraw_all();
    }

    /// Focus a window from an activation request: switch to its workspace, unminimize.
    pub fn activate_window(&mut self, window: &Window) {
        if window.data().minimized {
            self.set_minimized(window, false);
        }
        let ws = window.data().workspace;
        if !self.hecate.wm.is_workspace_active(ws) {
            let output = self
                .hecate
                .wm
                .workspace(ws)
                .and_then(|w| w.output.clone())
                .and_then(|n| self.hecate.wm.output_by_name(&n).cloned())
                .or_else(|| self.hecate.wm.focused_output.clone());
            if let Some(o) = output {
                self.hecate.wm.activate_workspace(&o, ws);
            }
        }
        self.focus_window(window);
    }

    /// Give focus to the topmost window on the focused output, or clear it.
    pub fn focus_topmost(&mut self) {
        if self.hecate.wm.is_locked() {
            return;
        }
        // Keep focus on an exclusive layer surface if there is one.
        if let Some(layer) = self.exclusive_layer_focus() {
            let keyboard = self.hecate.seat.get_keyboard().unwrap();
            keyboard.set_focus(self, Some(layer), SERIAL_COUNTER.next_serial());
            return;
        }
        let output = self
            .hecate
            .wm
            .focused_output
            .clone()
            .or_else(|| self.hecate.wm.outputs().next().cloned());
        let top = output.and_then(|o| {
            let mru = self.hecate.wm.mru();
            let on_output = self.hecate.wm.windows_on_output(&o);
            mru.into_iter().find(|w| on_output.contains(w))
        });
        match top {
            Some(w) => self.focus_window(&w),
            None => {
                if let Some(prev) = self.focused_window() {
                    prev.set_activated_state(false);
                }
                let keyboard = self.hecate.seat.get_keyboard().unwrap();
                keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
            }
        }
    }

    fn exclusive_layer_focus(
        &self,
    ) -> Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface> {
        use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer};
        for output in self.hecate.wm.outputs() {
            let map = smithay::desktop::layer_map_for_output(output);
            for layer in [Layer::Overlay, Layer::Top] {
                if let Some(l) = map.layers_on(layer).rev().find(|l| {
                    l.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                        && crate::utils::is_mapped(l.wl_surface())
                }) {
                    return Some(l.wl_surface().clone());
                }
            }
        }
        None
    }

    // ------------------------------------------------------------ geometry

    /// The rect a maximized window occupies.
    pub fn maximized_rect(
        &self,
        output: &Output,
        how: Maximized,
        current: Option<Rectangle<i32, Logical>>,
    ) -> Rectangle<i32, Logical> {
        let area = Wm::usable_area(output, &self.hecate.config);
        let gap = self.hecate.config.general.gaps;
        let bw = if self.hecate.config.border.keep_on_maximize {
            self.hecate.config.border.width.ceil() as i32
        } else {
            0
        };
        let full = geometry::shrink(area, gap + bw);
        let cur = current.unwrap_or(full);
        match how {
            Maximized::Both | Maximized::No => full,
            Maximized::Horizontal => {
                Rectangle::new((full.loc.x, cur.loc.y).into(), (full.size.w, cur.size.h).into())
            }
            Maximized::Vertical => {
                Rectangle::new((cur.loc.x, full.loc.y).into(), (cur.size.w, full.size.h).into())
            }
        }
    }

    /// Move and resize a window to a rect (geometry, border excluded).
    pub fn set_window_rect(&mut self, window: &Window, rect: Rectangle<i32, Logical>) {
        window.data().loc = rect.loc;
        window.request_size(Some(rect.size));
        self.hecate.queue_redraw_all();
    }

    fn save_restore(&self, window: &Window) {
        let rect = self.hecate.wm.window_rect(window);
        let mut d = window.data();
        if d.restore.is_none() {
            d.restore = Some(rect);
        }
    }

    fn take_restore(&mut self, window: &Window) {
        let restore = window.data().restore.take();
        match restore {
            Some(r) => self.set_window_rect(window, r),
            None => window.request_size(None),
        }
    }

    fn set_tiled_states(window: &Window, tiled: bool) {
        window.toplevel_surface().with_pending_state(|s| {
            for st in [
                xdg_toplevel::State::TiledLeft,
                xdg_toplevel::State::TiledRight,
                xdg_toplevel::State::TiledTop,
                xdg_toplevel::State::TiledBottom,
            ] {
                if tiled {
                    s.states.set(st);
                } else {
                    s.states.unset(st);
                }
            }
        });
    }

    pub fn set_maximized(&mut self, window: &Window, how: Maximized) {
        let Some(output) = self.hecate.wm.output_for_window(window).cloned() else {
            return;
        };
        let current = window.data().maximized;
        if current == how {
            return;
        }
        if how == Maximized::No {
            window.data().maximized = Maximized::No;
            window
                .toplevel_surface()
                .with_pending_state(|s| s.states.unset(xdg_toplevel::State::Maximized));
            Self::set_tiled_states(window, false);
            self.take_restore(window);
        } else {
            if window.data().fullscreen {
                return;
            }
            self.save_restore(window);
            let cur = window.data().restore;
            let rect = self.maximized_rect(&output, how, cur);
            {
                let mut d = window.data();
                d.maximized = how;
                d.snapped = None;
            }
            window.toplevel_surface().with_pending_state(|s| {
                if how == Maximized::Both {
                    s.states.set(xdg_toplevel::State::Maximized);
                }
            });
            Self::set_tiled_states(window, how != Maximized::Both);
            self.set_window_rect(window, rect);
        }
    }

    pub fn set_fullscreen(&mut self, window: &Window, on: bool, output: Option<Output>) {
        let output = output.or_else(|| self.hecate.wm.output_for_window(window).cloned());
        let Some(output) = output else {
            return;
        };
        if window.data().fullscreen == on {
            return;
        }
        if on {
            self.save_restore(window);
            window.data().fullscreen = true;
            let geo = Wm::output_geometry(&output);
            window.toplevel_surface().with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Fullscreen);
                s.fullscreen_output = window
                    .toplevel_surface()
                    .wl_surface()
                    .client()
                    .and_then(|c| output.client_outputs(&c).next());
            });
            self.set_window_rect(window, geo);
            self.hecate.wm.raise(window);
        } else {
            window.data().fullscreen = false;
            window.toplevel_surface().with_pending_state(|s| {
                s.states.unset(xdg_toplevel::State::Fullscreen);
                s.fullscreen_output = None;
            });
            let maximized = window.data().maximized;
            if maximized != Maximized::No {
                let cur = window.data().restore;
                let rect = self.maximized_rect(&output, maximized, cur);
                self.set_window_rect(window, rect);
            } else {
                self.take_restore(window);
            }
        }
        self.hecate.wm.restack();
        self.refresh_idle_inhibit();
    }

    pub fn set_minimized(&mut self, window: &Window, on: bool) {
        let ws_id = window.data().workspace;
        if window.data().minimized == on {
            return;
        }
        window.data().minimized = on;
        if let Some(ws) = self.hecate.wm.workspace_mut(ws_id) {
            if on {
                ws.windows.retain(|w| w != window);
                ws.minimized.push(window.clone());
            } else {
                ws.minimized.retain(|w| w != window);
                ws.windows.push(window.clone());
            }
        }
        self.hecate.wm.restack();
        if on {
            window.set_activated_state(false);
            self.focus_topmost();
        } else {
            self.focus_window(window);
        }
        self.hecate.wm.workspaces_dirty = true;
        self.hecate.queue_redraw_all();
    }

    pub fn snap_to_edge(&mut self, window: &Window, edge: Edge) {
        let Some(output) = self.hecate.wm.output_for_window(window).cloned() else {
            return;
        };
        if window.data().fullscreen {
            return;
        }
        if edge == Edge::Top && self.hecate.config.snapping.top_maximize {
            self.set_maximized(window, Maximized::Both);
            return;
        }
        self.save_restore(window);
        let area = Wm::usable_area(&output, &self.hecate.config);
        let gap = self.hecate.config.general.gaps;
        let rect = snap_rect(area, edge, gap);
        {
            let mut d = window.data();
            d.snapped = Some(Snap::Edge(edge));
            d.maximized = Maximized::No;
        }
        window
            .toplevel_surface()
            .with_pending_state(|s| s.states.unset(xdg_toplevel::State::Maximized));
        Self::set_tiled_states(window, true);
        self.set_window_rect(window, rect);
    }

    pub fn unsnap(&mut self, window: &Window) {
        let (snapped, maximized) = {
            let d = window.data();
            (d.snapped.is_some(), d.maximized)
        };
        if maximized != Maximized::No {
            self.set_maximized(window, Maximized::No);
        } else if snapped {
            window.data().snapped = None;
            Self::set_tiled_states(window, false);
            self.take_restore(window);
        }
    }

    /// Re-apply maximize/fullscreen/snap geometry after the usable area changed.
    pub fn on_usable_area_changed(&mut self, output: &Output) {
        let windows: Vec<Window> = self
            .hecate
            .wm
            .all_windows()
            .filter(|w| self.hecate.wm.output_for_window(w) == Some(output))
            .cloned()
            .collect();
        let area = Wm::usable_area(output, &self.hecate.config);
        for w in windows {
            let (fullscreen, maximized, snapped, restore) = {
                let d = w.data();
                (d.fullscreen, d.maximized, d.snapped, d.restore)
            };
            if fullscreen {
                let geo = Wm::output_geometry(output);
                self.set_window_rect(&w, geo);
            } else if maximized != Maximized::No {
                let rect = self.maximized_rect(output, maximized, restore);
                self.set_window_rect(&w, rect);
            } else if let Some(Snap::Edge(edge)) = snapped {
                let rect = snap_rect(area, edge, self.hecate.config.general.gaps);
                self.set_window_rect(&w, rect);
            }
            w.toplevel_surface()
                .with_pending_state(|s| s.bounds = Some(area.size));
        }
        self.hecate.queue_redraw(output);
    }

    pub fn center_window(&mut self, window: &Window) {
        let Some(output) = self.hecate.wm.output_for_window(window).cloned() else {
            return;
        };
        let area = Wm::usable_area(&output, &self.hecate.config);
        let size = window.geometry().size;
        window.data().loc = centered(area, size);
        self.hecate.queue_redraw_all();
    }

    pub fn cycle_preset(&mut self, window: &Window, horizontal: bool) {
        let Some(output) = self.hecate.wm.output_for_window(window).cloned() else {
            return;
        };
        let presets = if horizontal {
            self.hecate.config.sizing.width_presets.clone()
        } else {
            self.hecate.config.sizing.height_presets.clone()
        };
        if presets.is_empty() {
            return;
        }
        if window.data().maximized != Maximized::No {
            self.set_maximized(window, Maximized::No);
        }
        let area = Wm::usable_area(&output, &self.hecate.config);
        let gap = self.hecate.config.general.gaps;
        let next = {
            let mut d = window.data();
            let slot = if horizontal {
                &mut d.width_preset
            } else {
                &mut d.height_preset
            };
            let n = slot.map(|i| (i + 1) % presets.len()).unwrap_or(0);
            *slot = Some(n);
            n
        };
        let cur = self.hecate.wm.window_rect(window);
        let mut size = cur.size;
        if horizontal {
            size.w = ((area.size.w - 2 * gap) as f64 * presets[next]).round() as i32;
        } else {
            size.h = ((area.size.h - 2 * gap) as f64 * presets[next]).round() as i32;
        }
        let size = clamp_size(window, size);
        let mut loc = cur.loc;
        if horizontal {
            loc.x = area.loc.x + (area.size.w - size.w) / 2;
        } else {
            loc.y = area.loc.y + (area.size.h - size.h) / 2;
        }
        self.set_window_rect(window, Rectangle::new(loc, size));
    }

    pub fn start_interactive_move(&mut self, window: &Window, serial: Option<smithay::utils::Serial>) {
        crate::input::grabs::start_move(self, window, serial);
    }

    pub fn start_interactive_resize(
        &mut self,
        window: &Window,
        edge: ResizeEdge,
        serial: Option<smithay::utils::Serial>,
    ) {
        crate::input::grabs::start_resize(self, window, edge, serial);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WindowMatch;

    fn rule(app_id: Option<&str>, title: Option<&str>) -> WindowRule {
        WindowRule {
            matches: WindowMatch {
                app_id: app_id.map(String::from),
                title: title.map(String::from),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn matching() {
        assert!(rule(Some("steam_app_*"), None).matches.app_id.is_some());
        assert!(rule_matches(
            &rule(Some("steam_app_*"), None),
            "steam_app_1234",
            "x",
            None
        ));
        assert!(!rule_matches(
            &rule(Some("steam_app_*"), None),
            "steam",
            "x",
            None
        ));
        assert!(rule_matches(
            &rule(Some("zen-beta"), Some("Picture-in-Picture")),
            "zen-beta",
            "Picture-in-Picture",
            None
        ));
        assert!(!rule_matches(
            &rule(Some("zen-beta"), Some("Picture-in-Picture")),
            "zen-beta",
            "Mail",
            None
        ));
        assert!(rule_matches(&rule(None, None), "anything", "", None));
    }
}
