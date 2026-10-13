//! Executing actions from binds and IPC.

use smithay::desktop::Window;
use smithay::reexports::wayland_server::Resource;

use crate::config::action::{Action, Direction};
use crate::state::State;
use crate::wm::{Maximized, WindowExt, Wm};

impl State {
    /// Run an action against the focused window (or `target` if given).
    pub fn do_action(&mut self, action: Action, target: Option<Window>) {
        let window = target.or_else(|| self.focused_window());
        match action {
            Action::Exit => {
                tracing::info!("exit requested");
                self.hecate.stop_signal.stop();
            }
            Action::Reload => self.reload_config(),
            Action::Spawn(argv) => self.hecate.spawn(&argv),
            Action::SpawnSh(cmd) => self.hecate.spawn(&["sh".into(), "-c".into(), cmd]),
            Action::Terminal => {
                let argv = crate::utils::terminal_command(self.hecate.config.general.terminal.as_ref());
                self.hecate.spawn(&argv);
            }
            Action::ToggleKeybinds => {
                self.hecate.keybinds_enabled = !self.hecate.keybinds_enabled;
                tracing::info!(
                    "keybinds {}",
                    if self.hecate.keybinds_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            Action::PowerOffOutputs => {
                self.backend.set_outputs_powered(&mut self.hecate, false);
                // Any input turns them back on.
                self.hecate.wake_outputs_on_input = true;
            }
            Action::PowerOnOutputs => {
                self.backend.set_outputs_powered(&mut self.hecate, true);
                self.hecate.queue_redraw_all();
            }
            Action::Signal(name) => {
                self.ipc_broadcast_event(hecate_ipc::Event::Signal { name });
            }

            Action::Close => {
                if let Some(w) = window {
                    w.toplevel_surface().send_close();
                }
            }
            Action::Kill => {
                if let Some(w) = window
                    && let Some(client) = w.toplevel_surface().wl_surface().client()
                    && let Ok(creds) = client.get_credentials(&self.hecate.display_handle)
                {
                    // SAFETY: plain kill(2).
                    unsafe {
                        libc::kill(creds.pid, libc::SIGKILL);
                    }
                }
            }
            Action::Focus => {
                if let Some(w) = window {
                    self.activate_window(&w);
                }
            }
            Action::Raise => {
                if let Some(w) = window {
                    self.hecate.wm.raise(&w);
                    self.hecate.queue_redraw_all();
                }
            }
            Action::Lower => {
                if let Some(w) = window {
                    self.hecate.wm.lower(&w);
                    self.focus_topmost();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::Minimize => {
                if let Some(w) = window {
                    self.set_minimized(&w, true);
                }
            }
            Action::ToggleMaximize => self.toggle_max(window, Maximized::Both),
            Action::ToggleMaximizeHorizontal => self.toggle_max(window, Maximized::Horizontal),
            Action::ToggleMaximizeVertical => self.toggle_max(window, Maximized::Vertical),
            Action::Maximize => {
                if let Some(w) = window {
                    self.set_maximized(&w, Maximized::Both);
                }
            }
            Action::Unmaximize => {
                if let Some(w) = window {
                    self.set_maximized(&w, Maximized::No);
                }
            }
            Action::ToggleFullscreen => {
                if let Some(w) = window {
                    let on = !w.data().fullscreen;
                    self.set_fullscreen(&w, on, None);
                }
            }
            Action::ToggleAlwaysOnTop => {
                if let Some(w) = window {
                    let mut d = w.data();
                    d.always_on_top = !d.always_on_top;
                    if d.always_on_top {
                        d.always_on_bottom = false;
                    }
                    drop(d);
                    self.hecate.wm.restack();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::ToggleAlwaysOnBottom => {
                if let Some(w) = window {
                    let mut d = w.data();
                    d.always_on_bottom = !d.always_on_bottom;
                    if d.always_on_bottom {
                        d.always_on_top = false;
                    }
                    drop(d);
                    self.hecate.wm.restack();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::ToggleOmnipresent => {
                if let Some(w) = window {
                    let mut d = w.data();
                    d.omnipresent = !d.omnipresent;
                    drop(d);
                    self.hecate.wm.workspaces_dirty = true;
                }
            }
            Action::ToggleBorder => {
                if let Some(w) = window {
                    let mut d = w.data();
                    d.border_hidden = !d.border_hidden;
                    drop(d);
                    self.hecate.queue_redraw_all();
                }
            }
            Action::Center => {
                if let Some(w) = window {
                    self.center_window(&w);
                }
            }
            Action::CycleWidthPreset => {
                if let Some(w) = window {
                    self.cycle_preset(&w, true);
                }
            }
            Action::CycleHeightPreset => {
                if let Some(w) = window {
                    self.cycle_preset(&w, false);
                }
            }
            Action::ResetSize => {
                if let Some(w) = window {
                    {
                        let mut d = w.data();
                        d.width_preset = None;
                        d.height_preset = None;
                    }
                    self.unsnap(&w);
                    w.request_size(None);
                }
            }
            Action::ResizeRelative(rr) => {
                if let Some(w) = window
                    && let Some(output) = self.hecate.wm.output_for_window(&w).cloned()
                {
                    let area = Wm::usable_area(&output, &self.hecate.config);
                    let mut rect = self.hecate.wm.window_rect(&w);
                    if let Some(dw) = rr.width {
                        rect.size.w += dw.resolve(area.size.w);
                    }
                    if let Some(dh) = rr.height {
                        rect.size.h += dh.resolve(area.size.h);
                    }
                    rect.size = crate::wm::window::clamp_size(&w, rect.size);
                    self.set_window_rect(&w, rect);
                }
            }
            Action::MoveRelative(d) => {
                if let Some(w) = window {
                    let loc = w.data().loc;
                    w.data().loc = (loc.x + d.x, loc.y + d.y).into();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::MoveTo(p) => {
                if let Some(w) = window
                    && let Some(output) = self.hecate.wm.output_for_window(&w).cloned()
                {
                    let og = Wm::output_geometry(&output);
                    w.data().loc = (og.loc.x + p.x, og.loc.y + p.y).into();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::ResizeTo(s) => {
                if let Some(w) = window {
                    let size = crate::wm::window::clamp_size(&w, (s.width, s.height).into());
                    w.request_size(Some(size));
                }
            }
            Action::SnapToEdge(edge) => {
                if let Some(w) = window {
                    self.snap_to_edge(&w, edge);
                }
            }
            Action::Unsnap => {
                if let Some(w) = window {
                    self.unsnap(&w);
                }
            }
            Action::FocusDirection(dir) => self.focus_direction(dir),
            Action::FocusNext => self.focus_cycle(true),
            Action::FocusPrev => self.focus_cycle(false),
            Action::Move => {
                if let Some(w) = window {
                    self.start_interactive_move(&w, None);
                }
            }
            Action::Resize => {
                if let Some(w) = window {
                    let edge = crate::input::grabs::edge_for_pointer(self, &w);
                    self.start_interactive_resize(&w, edge, None);
                }
            }

            Action::Workspace(r) => {
                let Some(output) = self.current_output() else {
                    return;
                };
                if let Some(id) = self.hecate.wm.resolve_workspace(&output, &r) {
                    self.switch_workspace(&output, id);
                }
            }
            Action::MoveToWorkspace(r) => {
                let Some(w) = window else {
                    return;
                };
                let Some(output) = self.current_output() else {
                    return;
                };
                if let Some(id) = self.hecate.wm.resolve_workspace(&output, &r) {
                    self.hecate.wm.move_window_to_workspace(&w, id);
                    self.focus_topmost();
                    self.hecate.queue_redraw_all();
                }
            }
            Action::MoveWorkspaceToOutput(r) => {
                if self.hecate.wm.scope != crate::config::WorkspaceScope::PerOutput {
                    return;
                }
                let Some(from) = self.current_output() else {
                    return;
                };
                let Some(to) = self.hecate.wm.resolve_output(&from, &r.0) else {
                    return;
                };
                let ws_id = self.hecate.wm.active_workspace_id(&from);
                let windows: Vec<Window> = self
                    .hecate
                    .wm
                    .workspace(ws_id)
                    .map(|ws| ws.windows.clone())
                    .unwrap_or_default();
                let target = self.hecate.wm.active_workspace_id(&to);
                for w in windows {
                    self.hecate.wm.move_window_to_workspace(&w, target);
                }
                self.hecate.wm.focused_output = Some(to);
                self.focus_topmost();
                self.hecate.queue_redraw_all();
            }
            Action::ToggleShowDesktop => {
                self.hecate.wm.show_desktop = !self.hecate.wm.show_desktop;
                self.focus_topmost();
                self.hecate.queue_redraw_all();
            }

            Action::FocusOutput(r) => {
                let Some(from) = self.current_output() else {
                    return;
                };
                if let Some(to) = self.hecate.wm.resolve_output(&from, &r.0) {
                    self.hecate.wm.focused_output = Some(to.clone());
                    let center = crate::wm::geometry::center(Wm::output_geometry(&to)).to_f64();
                    self.warp_pointer(center);
                    self.focus_topmost();
                }
            }
            Action::MoveToOutput(r) => {
                let Some(w) = window else {
                    return;
                };
                let Some(from) = self.hecate.wm.output_for_window(&w).cloned() else {
                    return;
                };
                if let Some(to) = self.hecate.wm.resolve_output(&from, &r.0) {
                    let target_ws = self.hecate.wm.active_workspace_id(&to);
                    if self.hecate.wm.scope == crate::config::WorkspaceScope::PerOutput {
                        self.hecate.wm.move_window_to_workspace(&w, target_ws);
                    }
                    self.hecate.wm.translate_between_outputs(&w, &from, &to);
                    self.hecate.wm.focused_output = Some(to.clone());
                    self.on_usable_area_changed(&to);
                    self.focus_window(&w);
                    self.hecate.queue_redraw_all();
                }
            }
            Action::MoveCursor(p) => self.warp_pointer((p.x, p.y).into()),

            Action::SwitchKeyboardLayout(r) => self.switch_layout(r),
            Action::SetCursorTheme(t) => {
                self.hecate.config.cursor.theme = t.name.clone();
                self.hecate.config.cursor.size = t.size;
                self.hecate.cursor_manager = crate::cursor::CursorManager::new(&t.name, t.size);
                crate::utils::set_child_env("XCURSOR_THEME", Some(&t.name));
                crate::utils::set_child_env("XCURSOR_SIZE", Some(&t.size.to_string()));
                self.hecate.queue_redraw_all();
            }
        }
    }

    fn toggle_max(&mut self, window: Option<Window>, how: Maximized) {
        if let Some(w) = window {
            let cur = w.data().maximized;
            self.set_maximized(&w, if cur == how { Maximized::No } else { how });
        }
    }

    /// The output actions apply to: focused output, else the one under the pointer.
    pub fn current_output(&self) -> Option<smithay::output::Output> {
        self.hecate
            .wm
            .focused_output
            .clone()
            .or_else(|| self.hecate.wm.output_at(self.hecate.pointer_location).cloned())
            .or_else(|| self.hecate.wm.outputs().next().cloned())
    }

    /// Show workspace `id` on `output` and refocus.
    pub fn switch_workspace(&mut self, output: &smithay::output::Output, id: u64) {
        if self.hecate.wm.activate_workspace(output, id) {
            self.focus_topmost();
            self.refresh_idle_inhibit();
            self.hecate.queue_redraw_all();
        }
    }

    /// Activate a workspace on the output it belongs to (ext-workspace, IPC). No back-and-forth.
    pub fn activate_workspace_by_id(&mut self, id: u64) {
        if self.hecate.wm.is_workspace_active(id) {
            return;
        }
        let Some(ws) = self.hecate.wm.workspace(id) else {
            return;
        };
        let output = match &ws.output {
            Some(name) => self.hecate.wm.output_by_name(name).cloned(),
            None => self.current_output(),
        };
        if let Some(output) = output {
            self.switch_workspace(&output, id);
        }
    }

    fn focus_direction(&mut self, dir: Direction) {
        let Some(current) = self.focused_window() else {
            self.focus_topmost();
            return;
        };
        let cr = self.hecate.wm.window_rect(&current);
        let cc = crate::wm::geometry::center(cr);
        let Some(output) = self.hecate.wm.output_for_window(&current).cloned() else {
            return;
        };
        let best = self
            .hecate
            .wm
            .windows_on_output(&output)
            .into_iter()
            .filter(|w| w != &current)
            .filter_map(|w| {
                let c = crate::wm::geometry::center(self.hecate.wm.window_rect(&w));
                let (dx, dy) = ((c.x - cc.x) as i64, (c.y - cc.y) as i64);
                let ok = match dir {
                    Direction::Left => dx < 0,
                    Direction::Right => dx > 0,
                    Direction::Up => dy < 0,
                    Direction::Down => dy > 0,
                };
                // Penalise off-axis distance so "left" prefers windows level with us.
                let (along, across) = match dir {
                    Direction::Left | Direction::Right => (dx.abs(), dy.abs()),
                    Direction::Up | Direction::Down => (dy.abs(), dx.abs()),
                };
                ok.then_some((w, along + across * 2))
            })
            .min_by_key(|(_, d)| *d)
            .map(|(w, _)| w);
        if let Some(w) = best {
            self.focus_window(&w);
        }
    }

    fn focus_cycle(&mut self, forward: bool) {
        let Some(output) = self.current_output() else {
            return;
        };
        let on_output = self.hecate.wm.windows_on_output(&output);
        // Stable order: by window id, so repeated presses walk all windows.
        let mut ids: Vec<Window> = on_output;
        ids.sort_by_key(|w| w.wid());
        if ids.is_empty() {
            return;
        }
        let cur = self.focused_window();
        let idx = cur.and_then(|c| ids.iter().position(|w| *w == c));
        let next = match idx {
            Some(i) if forward => (i + 1) % ids.len(),
            Some(i) => (i + ids.len() - 1) % ids.len(),
            None => 0,
        };
        let w = ids[next].clone();
        self.focus_window(&w);
    }

    pub fn reload_config(&mut self) {
        let path = self.hecate.config_path.clone();
        let result = match &path {
            Some(p) if p.exists() => crate::config::load_file(p),
            _ => Ok(crate::config::Config::default()),
        };
        match result {
            Ok(config) => {
                tracing::info!("config reloaded");
                self.hecate.last_config_error = None;
                let effective = self.effective_config(&config);
                self.hecate.file_config = config;
                self.apply_config(effective);
                self.ipc_broadcast_event(hecate_ipc::Event::ConfigReloaded { failed: false });
            }
            Err(err) => {
                tracing::warn!("config reload failed, keeping the current config: {err}");
                self.hecate.config_reload_failures += 1;
                self.hecate.last_config_error = Some(err.to_string());
                self.ipc_broadcast_event(hecate_ipc::Event::ConfigReloaded { failed: true });
                if let Some(cmd) = self.hecate.config.general.error_command.clone() {
                    let msg = err.to_string();
                    let argv: Vec<String> = cmd.iter().map(|a| a.replace("{error}", &msg)).collect();
                    self.hecate.spawn(&argv);
                }
            }
        }
    }

    pub fn apply_config(&mut self, config: crate::config::Config) {
        let old = std::mem::replace(&mut self.hecate.config, config);
        self.hecate.binds = crate::config::load::resolved_binds(&self.hecate.config);
        self.hecate.mouse_binds = crate::config::load::resolved_mouse_binds(&self.hecate.config);
        for (k, v) in &self.hecate.config.environment {
            crate::utils::set_child_env(k, Some(v));
        }
        if old.keyboard != self.hecate.config.keyboard {
            self.apply_keyboard_config();
        }
        if old.cursor.theme != self.hecate.config.cursor.theme
            || old.cursor.size != self.hecate.config.cursor.size
        {
            let c = &self.hecate.config.cursor;
            self.hecate.cursor_manager = crate::cursor::CursorManager::new(&c.theme, c.size);
        }
        self.hecate.wm.anim = self.hecate.config.animations.clone();
        self.hecate
            .background_effect_state
            .set_blur(self.hecate.config.blur.enable);
        if old.workspaces != self.hecate.config.workspaces {
            let cfg = self.hecate.config.clone();
            self.hecate.wm.ensure_workspaces(&cfg);
        }
        if old.outputs != self.hecate.config.outputs {
            self.backend.apply_output_config(&mut self.hecate);
        }
        if old.xwayland != self.hecate.config.xwayland {
            self.teardown_xwayland();
            self.setup_xwayland();
        }
        if old.input != self.hecate.config.input {
            self.backend.apply_input_config(&self.hecate.config);
        }
        let windows: Vec<Window> = self.hecate.wm.all_windows().cloned().collect();
        for w in windows {
            self.apply_window_rules(&w, false);
        }
        let outputs: Vec<_> = self.hecate.wm.outputs().cloned().collect();
        for o in outputs {
            self.on_usable_area_changed(&o);
        }
        self.hecate.queue_redraw_all();
    }
}
