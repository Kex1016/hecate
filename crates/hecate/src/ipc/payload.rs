//! Building IPC payloads from compositor state.

use std::collections::BTreeMap;

use hecate_ipc as ipc;
use smithay::desktop::{Window, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer};

use crate::state::State;
use crate::wm::{Maximized, WindowExt, Wm};

fn rect(r: smithay::utils::Rectangle<i32, smithay::utils::Logical>) -> ipc::Rect {
    ipc::Rect {
        x: r.loc.x,
        y: r.loc.y,
        width: r.size.w,
        height: r.size.h,
    }
}

impl State {
    pub fn ipc_window(&self, w: &Window) -> ipc::Window {
        let wm = &self.hecate.wm;
        let geo = wm.window_rect(w);
        let focused = self.focused_window().as_ref() == Some(w);
        let d = w.data();
        let ws = d.workspace;
        let stacking = wm
            .workspace(ws)
            .and_then(|ws| ws.windows.iter().position(|x| x == w))
            .unwrap_or(0);
        let pid = w
            .toplevel()
            .and_then(|t| t.wl_surface().client())
            .and_then(|c| c.get_credentials(&self.hecate.display_handle).ok())
            .map(|c| c.pid);
        let app_id = w.app_id();
        let title = w.title();
        let output = drop_and(d, || wm.output_for_window(w).map(|o| o.name()));
        let d = w.data();
        ipc::Window {
            id: d.id,
            title: (!title.is_empty()).then_some(title),
            app_id: (!app_id.is_empty()).then_some(app_id),
            pid,
            workspace_id: Some(ws),
            is_focused: focused,
            is_floating: true,
            is_urgent: d.urgent,
            layout: ipc::WindowLayout {
                window_size: (geo.size.w, geo.size.h),
                tile_size: (geo.size.w as f64, geo.size.h as f64),
                pos_in_scrolling_layout: None,
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0., 0.),
            },
            output,
            geometry: rect(geo),
            stacking,
            is_maximized: d.maximized != Maximized::No,
            is_fullscreen: d.fullscreen,
            is_minimized: d.minimized,
            always_on_top: d.always_on_top,
            always_on_bottom: d.always_on_bottom,
            omnipresent: d.omnipresent,
            is_x11: false,
            focus_serial: d.focus_serial,
        }
    }

    pub fn ipc_windows(&self) -> Vec<ipc::Window> {
        let mut windows: Vec<Window> = self
            .hecate
            .wm
            .workspaces
            .iter()
            .flat_map(|ws| ws.windows.iter().chain(ws.minimized.iter()))
            .cloned()
            .collect();
        windows.sort_by_key(|w| w.wid());
        windows.iter().map(|w| self.ipc_window(w)).collect()
    }

    pub fn ipc_workspaces(&self) -> Vec<ipc::Workspace> {
        let wm = &self.hecate.wm;
        let focused_output = wm.focused_output.clone();
        let focused_ws = focused_output.as_ref().map(|o| wm.active_workspace_id(o));
        let focused_window = self.focused_window();
        wm.workspaces
            .iter()
            .map(|ws| {
                let output = ws.output.clone().or_else(|| {
                    // Global scope: report the focused output (that's where it's shown).
                    focused_output.as_ref().map(|o| o.name())
                });
                let active_window = focused_window
                    .as_ref()
                    .filter(|w| ws.windows.contains(w))
                    .or_else(|| ws.windows.last())
                    .map(|w| w.wid());
                ipc::Workspace {
                    id: ws.id,
                    idx: wm.workspace_index(ws.id).unwrap_or(0) as u8,
                    name: ws.name.clone(),
                    output,
                    is_urgent: ws.windows.iter().any(|w| w.data().urgent),
                    is_active: wm.is_workspace_active(ws.id),
                    is_focused: focused_ws == Some(ws.id),
                    active_window_id: active_window,
                    window_ids: ws
                        .windows
                        .iter()
                        .chain(ws.minimized.iter())
                        .map(|w| w.wid())
                        .collect(),
                }
            })
            .collect()
    }

    pub fn ipc_output(&self, o: &Output) -> ipc::Output {
        let props = o.physical_properties();
        let current = o.current_mode();
        let preferred = o.preferred_mode();
        let modes: Vec<smithay::output::Mode> = o.modes();
        let geo = Wm::output_geometry(o);
        let transform = format!("{:?}", o.current_transform()).to_lowercase();
        ipc::Output {
            name: o.name(),
            make: props.make,
            model: props.model,
            serial: Some(props.serial_number),
            physical_size: Some((props.size.w.max(0) as u32, props.size.h.max(0) as u32)),
            current_mode: current.and_then(|c| modes.iter().position(|m| *m == c)),
            modes: modes
                .iter()
                .map(|m| ipc::Mode {
                    width: m.size.w as u16,
                    height: m.size.h as u16,
                    refresh_rate: m.refresh as u32,
                    is_preferred: Some(*m) == preferred,
                })
                .collect(),
            vrr_supported: false,
            vrr_enabled: false,
            logical: Some(ipc::LogicalOutput {
                x: geo.loc.x,
                y: geo.loc.y,
                width: geo.size.w as u32,
                height: geo.size.h as u32,
                scale: o.current_scale().fractional_scale(),
                transform: transform.trim_start_matches('_').to_string(),
            }),
            is_focused: self.hecate.wm.focused_output.as_ref() == Some(o),
            usable_area: Some(rect(Wm::usable_area(o, &self.hecate.config))),
        }
    }

    pub fn ipc_outputs(&self) -> BTreeMap<String, ipc::Output> {
        self.hecate
            .wm
            .outputs()
            .map(|o| (o.name(), self.ipc_output(o)))
            .collect()
    }

    pub fn ipc_layers(&self) -> Vec<ipc::Layer> {
        let mut out = Vec::new();
        for o in self.hecate.wm.outputs() {
            let map = layer_map_for_output(o);
            let og = Wm::output_geometry(o);
            for l in map.layers() {
                let state = l.cached_state();
                let mut geo = map.layer_geometry(l).unwrap_or_default();
                geo.loc += og.loc;
                out.push(ipc::Layer {
                    namespace: l.namespace().to_string(),
                    layer: match l.layer() {
                        Layer::Background => "background",
                        Layer::Bottom => "bottom",
                        Layer::Top => "top",
                        Layer::Overlay => "overlay",
                    }
                    .to_string(),
                    output: o.name(),
                    geometry: rect(geo),
                    exclusive_zone: match state.exclusive_zone {
                        smithay::wayland::shell::wlr_layer::ExclusiveZone::Exclusive(n) => n as i32,
                        smithay::wayland::shell::wlr_layer::ExclusiveZone::Neutral => 0,
                        smithay::wayland::shell::wlr_layer::ExclusiveZone::DontCare => -1,
                    },
                    keyboard_interactivity: match state.keyboard_interactivity {
                        KeyboardInteractivity::None => "none",
                        KeyboardInteractivity::Exclusive => "exclusive",
                        KeyboardInteractivity::OnDemand => "on_demand",
                    }
                    .to_string(),
                });
            }
        }
        out
    }

    pub fn ipc_keyboard_layouts(&mut self) -> ipc::KeyboardLayouts {
        let keyboard = self.hecate.seat.get_keyboard().unwrap();
        keyboard.with_xkb_state(self, |ctx| {
            let xkb = ctx.xkb().lock().unwrap();
            let names = xkb.layouts().map(|l| xkb.layout_name(l).to_string()).collect();
            ipc::KeyboardLayouts {
                names,
                current_idx: xkb.active_layout().0 as u8,
            }
        })
    }

    pub fn ipc_binds(&self) -> Vec<ipc::Bind> {
        self.hecate
            .binds
            .iter()
            .map(|b| ipc::Bind {
                key: b.combo.to_string(),
                source: b.source.clone(),
                action: serde_json::to_value(&b.value).unwrap_or_default(),
                description: b.value.description.clone(),
            })
            .collect()
    }

    pub fn ipc_window_details(&self, w: &Window) -> ipc::WindowDetails {
        let window = self.ipc_window(w);
        let client_data = w.toplevel().and_then(|t| t.wl_surface().client()).and_then(|c| {
            c.get_data::<crate::state::ClientState>()
                .map(|d| d.security_context.clone())
        });
        let ctx = client_data.flatten();
        let surface = w.toplevel_surface().wl_surface().clone();
        let matched: Vec<(usize, serde_json::Value)> = w
            .data()
            .rules
            .matched
            .iter()
            .filter_map(|i| {
                self.hecate
                    .config
                    .window_rules
                    .get(*i)
                    .map(|r| (*i, serde_json::to_value(r).unwrap_or_default()))
            })
            .collect();
        let (toml, nix) = suggested_rule(window.app_id.as_deref(), window.title.as_deref());
        let decoration = match self.decoration_mode_for(w.toplevel_surface()) {
            smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode::ClientSide => "client",
            _ => "server",
        };
        ipc::WindowDetails {
            window,
            sandbox_engine: ctx.as_ref().and_then(|c| c.sandbox_engine.clone()),
            sandbox_app_id: ctx.as_ref().and_then(|c| c.app_id.clone()),
            decoration: decoration.into(),
            inhibits_idle: self.hecate.idle_inhibitors.contains(&surface),
            inhibits_shortcuts: self.hecate.shortcut_inhibitors.contains(&surface),
            tearing_requested: w.data().rules.tearing,
            matched_rules: matched,
            suggested_rule_toml: toml,
            suggested_rule_nix: nix,
        }
    }

    /// Resolve a selector to a window (`Pick` is handled by the caller).
    pub fn select_window(&self, sel: &ipc::Selector) -> Result<Window, String> {
        let windows: Vec<Window> = self.hecate.wm.all_windows().cloned().collect();
        let matches: Vec<Window> = match sel {
            ipc::Selector::Focused => self.focused_window().into_iter().collect(),
            ipc::Selector::Pick => return Err("pick must be handled asynchronously".into()),
            ipc::Selector::Id(id) => windows.into_iter().filter(|w| w.wid() == *id).collect(),
            ipc::Selector::AppId(pat) => {
                let g = glob::Pattern::new(pat).map_err(|e| e.to_string())?;
                windows.into_iter().filter(|w| g.matches(&w.app_id())).collect()
            }
            ipc::Selector::Title(pat) => {
                let g = glob::Pattern::new(pat).map_err(|e| e.to_string())?;
                windows.into_iter().filter(|w| g.matches(&w.title())).collect()
            }
            ipc::Selector::Pid(pid) => windows
                .into_iter()
                .filter(|w| self.ipc_window(w).pid == Some(*pid))
                .collect(),
        };
        match matches.len() {
            0 => Err("no matching window".into()),
            1 => Ok(matches.into_iter().next().unwrap()),
            n => Err(format!("{n} windows match; be more specific (e.g. by id)")),
        }
    }

    pub fn ipc_tree(&self) -> Vec<ipc::TreeOutput> {
        let workspaces = self.ipc_workspaces();
        let layers = self.ipc_layers();
        self.hecate
            .wm
            .outputs()
            .map(|o| {
                let name = o.name();
                let ws: Vec<ipc::TreeWorkspace> = self
                    .hecate
                    .wm
                    .workspaces_for(o)
                    .into_iter()
                    .filter_map(|w| {
                        let info = workspaces.iter().find(|x| x.id == w.id)?.clone();
                        Some(ipc::TreeWorkspace {
                            workspace: info,
                            windows: w.windows.iter().map(|x| self.ipc_window(x)).collect(),
                        })
                    })
                    .collect();
                ipc::TreeOutput {
                    layers: layers.iter().filter(|l| l.output == name).cloned().collect(),
                    name,
                    workspaces: ws,
                }
            })
            .collect()
    }

    pub fn ipc_clients(&self) -> Vec<ipc::ClientInfo> {
        let dh = &self.hecate.display_handle;
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for w in self.hecate.wm.all_windows() {
            let Some(client) = w.toplevel().and_then(|t| t.wl_surface().client()) else {
                continue;
            };
            if seen.contains(&client.id()) {
                continue;
            }
            seen.push(client.id());
            let pid = client.get_credentials(dh).ok().map(|c| c.pid);
            let exe = pid.and_then(|p| std::fs::read_link(format!("/proc/{p}/exe")).ok());
            let ctx = client
                .get_data::<crate::state::ClientState>()
                .and_then(|d| d.security_context.clone());
            out.push(ipc::ClientInfo {
                pid,
                executable: exe.map(|p| p.to_string_lossy().into_owned()),
                sandboxed: ctx.is_some(),
                sandbox_engine: ctx.and_then(|c| c.sandbox_engine),
            });
        }
        out
    }
}

fn drop_and<T, R>(guard: T, f: impl FnOnce() -> R) -> R {
    drop(guard);
    f()
}

/// A window rule that matches this window, as TOML and as Nix.
pub fn suggested_rule(app_id: Option<&str>, title: Option<&str>) -> (String, String) {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut toml_parts = Vec::new();
    let mut nix_parts = Vec::new();
    if let Some(a) = app_id {
        toml_parts.push(format!("app_id = \"{}\"", esc(a)));
        nix_parts.push(format!("app_id = \"{}\";", esc(a)));
    }
    if let Some(t) = title
        && app_id.is_none()
    {
        toml_parts.push(format!("title = \"{}\"", esc(t)));
        nix_parts.push(format!("title = \"{}\";", esc(t)));
    }
    let toml = format!(
        "[[window_rules]]\nmatch = {{ {} }}\n# always_on_top = true\n# border = {{ width = 0 }}\n",
        toml_parts.join(", ")
    );
    let nix = format!(
        "{{\n  match = {{ {} }};\n  # always_on_top = true;\n  # border.width = 0;\n}}\n",
        nix_parts.join(" ")
    );
    (toml, nix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_rules_parse() {
        let (toml, nix) = suggested_rule(Some("zen-beta"), Some("Picture-in-Picture"));
        let c = crate::config::load::from_toml_str(&toml).unwrap();
        assert_eq!(c.window_rules[0].matches.app_id.as_deref(), Some("zen-beta"));
        assert!(nix.contains("app_id = \"zen-beta\";"));
        let (toml, _) = suggested_rule(None, Some("He said \"hi\""));
        let c = crate::config::load::from_toml_str(&toml).unwrap();
        assert_eq!(c.window_rules[0].matches.title.as_deref(), Some("He said \"hi\""));
    }
}
