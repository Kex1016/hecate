use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel::State as XdgState;

use super::fixture::Fixture;
use crate::config::action::{Action, Direction, Edge};
use crate::wm::{Maximized, WindowExt, Wm};

const RED: [u8; 4] = [255, 0, 0, 255];

fn setup(toml: &str) -> (Fixture, super::client::ClientId) {
    let mut f = Fixture::with_toml(toml);
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    (f, id)
}

#[test]
fn window_maps_centered_and_focused() {
    let (mut f, id) = setup("");
    f.map_window(id, (400, 300), RED, Some("test"));
    let windows = f.windows();
    assert_eq!(windows.len(), 1, "window should be mapped");
    let w = &windows[0];
    let rect = f.hecate().hecate.wm.window_rect(w);
    assert_eq!(rect.size, (400, 300).into());
    assert_eq!(
        rect.loc,
        ((1920 - 400) / 2, (1080 - 300) / 2).into(),
        "center placement"
    );
    assert_eq!(f.focused().as_ref(), Some(w), "new window gets focus");
    assert_eq!(w.app_id(), "test");
}

#[test]
fn initial_configure_has_bounds_and_server_decorations_by_default() {
    let (mut f, id) = setup("");
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.bounds, Some((1920, 1080)), "bounds = usable area");
}

#[test]
fn second_window_is_focused_and_on_top() {
    let (mut f, id) = setup("");
    f.map_window(id, (400, 300), RED, Some("a"));
    f.map_window(id, (400, 300), RED, Some("b"));
    let ws = f.windows();
    let (a, b) = (ws[0].clone(), ws[1].clone());
    assert_eq!(f.focused(), Some(b.clone()));
    let output = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    let stack = f.hecate().hecate.wm.windows_on_output(&output);
    assert_eq!(stack.last(), Some(&b), "focused window is topmost");

    f.hecate().do_action(Action::FocusNext, None);
    let focused = f.focused().unwrap();
    assert_eq!(focused, a, "focus_next cycles");
}

#[test]
fn maximize_and_restore() {
    let (mut f, id) = setup("[border]\nkeep_on_maximize = false");
    let surface = f.map_window(id, (400, 300), RED, Some("a"));
    let w = f.windows()[0].clone();
    let before = f.hecate().hecate.wm.window_rect(&w);

    f.hecate().do_action(Action::ToggleMaximize, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (1920, 1080));
    assert!(cfg.states.contains(&XdgState::Maximized));
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(w.data().maximized, Maximized::Both);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w).loc, (0, 0).into());

    f.hecate().do_action(Action::ToggleMaximize, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert!(!cfg.states.contains(&XdgState::Maximized));
    assert_eq!(cfg.size, (before.size.w, before.size.h), "restores the old size");
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(
        f.hecate().hecate.wm.window_rect(&w),
        before,
        "restores the old rect"
    );
}

#[test]
fn maximize_respects_gaps() {
    let (mut f, id) = setup("[general]\ngaps = 10");
    let surface = f.map_window(id, (400, 300), RED, None);
    f.hecate().do_action(Action::Maximize, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (1900, 1060));
}

#[test]
fn edge_resistance_meets_border_to_border() {
    let (mut f, id) = setup("[border]\nwidth = 4\n[resistance]\nscreen_edge = 0");
    f.map_window(id, (400, 300), RED, None);
    f.map_window(id, (400, 300), RED, None);
    let ws = f.windows();
    let (a, b) = (ws[0].clone(), ws[1].clone());
    f.hecate().set_window_rect(
        &a,
        smithay::utils::Rectangle::new((100, 100).into(), (400, 300).into()),
    );
    // b dragged so its border starts 5 px right of a's border (a's outer right edge is x = 504).
    let got = crate::input::grabs::resist(f.hecate(), &b, (513, 100).into());
    assert_eq!(
        got,
        (508, 100).into(),
        "b's border meets a's, not b's window edge"
    );
}

#[test]
fn snap_left_and_right() {
    // The border is part of the snapped half: 4 px of it fits between the window and each edge.
    let (mut f, id) = setup("[border]\nwidth = 4");
    let surface = f.map_window(id, (400, 300), RED, None);
    let w = f.windows()[0].clone();
    f.hecate().do_action(Action::SnapToEdge(Edge::Left), None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (952, 1072));
    assert!(cfg.states.contains(&XdgState::TiledLeft));
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w).loc, (4, 4).into());

    f.hecate().do_action(Action::SnapToEdge(Edge::Right), None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w).loc, (964, 4).into());

    f.hecate().do_action(Action::Unsnap, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (400, 300), "unsnap restores");
}

#[test]
fn client_fullscreen_request() {
    let (mut f, id) = setup("");
    let surface = f.map_window(id, (400, 300), RED, None);
    let w = f.windows()[0].clone();
    f.client(id).window(&surface).set_fullscreen(None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (1920, 1080));
    assert!(cfg.states.contains(&XdgState::Fullscreen));
    assert!(w.data().fullscreen);

    f.client(id).window(&surface).unset_fullscreen();
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert!(!cfg.states.contains(&XdgState::Fullscreen));
    assert_eq!(cfg.size, (400, 300));
}

#[test]
fn width_presets_cycle() {
    let (mut f, id) = setup("[sizing]\nwidth_presets = [0.25, 0.5]");
    let surface = f.map_window(id, (400, 300), RED, None);
    f.hecate().do_action(Action::CycleWidthPreset, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size.0, 480);
    f.ack_and_resize(id, &surface, RED);
    f.hecate().do_action(Action::CycleWidthPreset, None);
    f.double_roundtrip(id);
    let cfg = f
        .client(id)
        .window(&surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size.0, 960);
}

#[test]
fn window_rules_apply() {
    let toml = r#"
[[window_rules]]
match = { app_id = "pip*" }
always_on_top = true
border = { width = 0 }

[[window_rules]]
match = { app_id = "*", title = "never" }
always_on_bottom = true
"#;
    let (mut f, id) = setup(toml);
    f.map_window(id, (200, 200), RED, Some("pip-player"));
    f.map_window(id, (400, 300), RED, Some("other"));
    let ws = f.windows();
    let (pip, other) = (ws[0].clone(), ws[1].clone());
    assert!(pip.data().always_on_top);
    assert!(!other.data().always_on_bottom, "second rule needs the title too");
    assert_eq!(pip.data().rules.matched, vec![0]);
    // The always-on-top window stays above the newer, focused one.
    let output = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    let stack = f.hecate().hecate.wm.windows_on_output(&output);
    assert_eq!(stack.last(), Some(&pip));
    assert_eq!(f.focused(), Some(other));
}

#[test]
fn focus_direction_picks_neighbour() {
    let (mut f, id) = setup("");
    let left = f.map_window(id, (300, 300), RED, Some("left"));
    let _ = left;
    f.hecate().do_action(Action::SnapToEdge(Edge::Left), None);
    f.double_roundtrip(id);
    let s = f.client(id).state.windows[0].surface.clone();
    f.ack_and_resize(id, &s, RED);
    let right = f.map_window(id, (300, 300), RED, Some("right"));
    f.hecate().do_action(Action::SnapToEdge(Edge::Right), None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &right, RED);

    let ws = f.windows();
    assert_eq!(f.focused(), Some(ws[1].clone()));
    f.hecate()
        .do_action(Action::FocusDirection(Direction::Left), None);
    assert_eq!(f.focused(), Some(ws[0].clone()));
    f.hecate()
        .do_action(Action::FocusDirection(Direction::Right), None);
    assert_eq!(f.focused(), Some(ws[1].clone()));
}

#[test]
fn close_action_asks_the_client() {
    let (mut f, id) = setup("");
    let surface = f.map_window(id, (300, 300), RED, None);
    f.hecate().do_action(Action::Close, None);
    f.double_roundtrip(id);
    assert!(f.client(id).window(&surface).close_requested);
}

#[test]
fn minimize_and_restore_focus() {
    let (mut f, id) = setup("");
    f.map_window(id, (300, 300), RED, Some("a"));
    f.map_window(id, (300, 300), RED, Some("b"));
    let ws = f.windows();
    f.hecate().do_action(Action::Minimize, None);
    assert!(ws[1].data().minimized);
    assert_eq!(f.focused(), Some(ws[0].clone()), "focus falls back");
    let output = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    assert!(!f.hecate().hecate.wm.windows_on_output(&output).contains(&ws[1]));
    f.hecate().activate_window(&ws[1]);
    assert!(!ws[1].data().minimized);
    assert_eq!(f.focused(), Some(ws[1].clone()));
}

#[test]
fn usable_area_matches_output() {
    let (mut f, _id) = setup("[[margins]]\ntop = 20\nleft = 5");
    let output = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    let area = Wm::usable_area(&output, &f.hecate().hecate.config);
    assert_eq!(area.loc, (5, 20).into());
    assert_eq!(area.size, (1915, 1060).into());
}

/// A resize from a left/top edge keeps the opposite edges where they were: the window grows
/// towards the pointer, not away from it.
#[test]
fn resizing_from_left_and_top_edges_moves_the_window() {
    use crate::wm::window::ResizeEdge;
    for (edge, grow) in [
        (ResizeEdge::Left, (100, 0)),
        (ResizeEdge::Top, (0, 80)),
        (ResizeEdge::TopLeft, (100, 80)),
        (ResizeEdge::BottomLeft, (100, 80)),
        (ResizeEdge::Right, (100, 0)),
        // Shrinking: the new geometry fits in the old bounding box.
        (ResizeEdge::Left, (-100, 0)),
        (ResizeEdge::TopLeft, (-100, -80)),
    ] {
        let (mut f, id) = setup("");
        let surface = f.map_window(id, (400, 300), RED, None);
        f.client(id)
            .window(&surface)
            .xdg_surface
            .set_window_geometry(0, 0, 400, 300);
        f.client(id).window(&surface).commit();
        f.double_roundtrip(id);
        let w = f.windows()[0].clone();
        let initial = f.hecate().hecate.wm.window_rect(&w);
        {
            let mut d = w.data();
            d.resize_edge = Some(edge);
            d.resize_initial = Some(initial);
            d.resizing = true;
        }
        w.request_size(Some((400 + grow.0, 300 + grow.1).into()));
        f.double_roundtrip(id);
        // Like GTK and Qt: an explicit window geometry, double-buffered with the commit.
        f.client(id)
            .window(&surface)
            .xdg_surface
            .set_window_geometry(0, 0, 400 + grow.0, 300 + grow.1);
        f.ack_and_resize(id, &surface, RED);

        let rect = f.hecate().hecate.wm.window_rect(&w);
        assert_eq!(rect.size, (400 + grow.0, 300 + grow.1).into(), "{edge:?}: size");
        let right = initial.loc.x + initial.size.w;
        let bottom = initial.loc.y + initial.size.h;
        if edge.left() {
            assert_eq!(rect.loc.x + rect.size.w, right, "{edge:?}: right edge stays put");
        } else {
            assert_eq!(rect.loc.x, initial.loc.x, "{edge:?}: left edge stays put");
        }
        if edge.top() {
            assert_eq!(
                rect.loc.y + rect.size.h,
                bottom,
                "{edge:?}: bottom edge stays put"
            );
        } else {
            assert_eq!(rect.loc.y, initial.loc.y, "{edge:?}: top edge stays put");
        }
    }
}

#[test]
fn interactive_resize_waits_for_the_client_to_draw_each_step() {
    let (mut f, id) = setup("");
    let surface = f.map_window(id, (400, 300), RED, None);
    f.double_roundtrip(id);
    let w = f.windows()[0].clone();
    w.data().resizing = true;
    let sizes = |f: &mut Fixture| -> Vec<(i32, i32)> {
        f.client(id)
            .window(&surface)
            .recent_configures()
            .map(|c| c.size)
            .collect()
    };
    sizes(&mut f);

    // Pointer motion is far faster than redraws: only the first step goes out.
    for s in [(450, 320), (480, 340), (500, 350)] {
        w.resize_step(s.into());
    }
    f.double_roundtrip(id);
    assert_eq!(sizes(&mut f), vec![(450, 320)]);

    // The client drew it: the latest size follows, the ones in between are skipped.
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(sizes(&mut f), vec![(500, 350)]);

    // Drawn too, and nothing newer: no more configures.
    f.ack_and_resize(id, &surface, RED);
    assert!(sizes(&mut f).is_empty());

    // A client that never draws doesn't freeze the drag.
    w.resize_step((520, 360).into());
    w.resize_step((540, 370).into());
    f.double_roundtrip(id);
    assert_eq!(sizes(&mut f), vec![(520, 360)]);
    std::thread::sleep(crate::wm::window::RESIZE_STEP_TIMEOUT + std::time::Duration::from_millis(20));
    w.resize_step((560, 380).into());
    f.double_roundtrip(id);
    assert_eq!(sizes(&mut f), vec![(560, 380)]);
}

#[test]
fn activating_a_window_before_it_maps_keeps_the_workspace() {
    let (mut f, id) = setup("");
    f.map_window(id, (300, 200), RED, None);
    let output = f.hecate().hecate.wm.outputs().next().cloned().unwrap();
    let ws = f.hecate().hecate.wm.active_workspace_id(&output);

    // A launcher's new app uses its activation token before its first buffer.
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.double_roundtrip(id);
    let pending = f.hecate().hecate.wm.pending.last().cloned().unwrap();
    f.hecate().activate_window(&pending);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ws);
    assert_eq!(f.hecate().hecate.wm.windows_on_output(&output).len(), 1);

    // Once it maps, the activation goes through: it lands here, focused.
    let window = f.client(id).window(&surface);
    window.attach_color(RED);
    window.set_size(200, 100);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ws);
    assert_eq!(f.hecate().hecate.wm.windows_on_output(&output).len(), 2);
    assert_eq!(f.focused(), Some(pending));
}
