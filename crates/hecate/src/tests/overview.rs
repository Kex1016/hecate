//! The window overview (Mod+W): state, keys, pointer and drawing.

use smithay::backend::input::{InputTime, KeyState};

use super::client::ClientId;
use super::fixture::Fixture;
use crate::config::action::{Action, WorkspaceRef};
use crate::overview::filter_matches;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const SUPER: u32 = 125 + 8;
const W: u32 = 17 + 8;

fn setup() -> (Fixture, ClientId) {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    (f, id)
}

fn key(f: &mut Fixture, id: ClientId, code: u32, state: KeyState) {
    f.hecate().handle_key(code.into(), state, InputTime::now());
    f.double_roundtrip(id);
}

fn tap(f: &mut Fixture, id: ClientId, codes: &[u32]) {
    for &c in codes {
        key(f, id, c, KeyState::Pressed);
    }
    for &c in codes.iter().rev() {
        key(f, id, c, KeyState::Released);
    }
}

fn entries(f: &mut Fixture) -> Vec<smithay::desktop::Window> {
    f.hecate()
        .hecate
        .overview
        .per_output
        .get("headless-1")
        .map(|e| e.iter().map(|e| e.window.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn mod_w_toggles() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.map_window(id, (300, 200), BLUE, Some("b"));
    tap(&mut f, id, &[SUPER, W]);
    assert!(f.hecate().hecate.overview.open);
    assert_eq!(entries(&mut f).len(), 2);
    f.hecate().close_overview(None);
    assert!(!f.hecate().hecate.overview.open);
    f.hecate().do_action(Action::ToggleOverview, None);
    assert!(f.hecate().hecate.overview.open, "the action opens it again");
    f.hecate().do_action(Action::ToggleOverview, None);
    assert!(!f.hecate().hecate.overview.open, "and closes it");
}

#[test]
fn opening_with_no_windows_does_nothing() {
    let (mut f, _) = setup();
    f.hecate().toggle_overview();
    assert!(!f.hecate().hecate.overview.open);
}

#[test]
fn minimized_windows_are_left_out() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.map_window(id, (300, 200), BLUE, Some("b"));
    let a = f.windows()[0].clone();
    f.hecate().set_minimized(&a, true);
    f.hecate().open_overview();
    assert_eq!(entries(&mut f), vec![f.windows()[1].clone()]);
}

#[test]
fn closing_without_choice_refocuses_the_previous_window() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.map_window(id, (300, 200), BLUE, Some("b"));
    let b = f.windows()[1].clone();
    f.hecate().focus_window(&b);
    f.hecate().open_overview();
    assert_eq!(
        f.hecate().hecate.overview.selected,
        Some(b.clone()),
        "opens on the focused window"
    );
    f.hecate().close_overview(None);
    assert_eq!(f.focused(), Some(b));
}

#[test]
fn closing_with_a_window_focuses_it() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.map_window(id, (300, 200), BLUE, Some("b"));
    let a = f.windows()[0].clone();
    f.hecate().open_overview();
    f.hecate().close_overview(Some(a.clone()));
    assert_eq!(f.focused(), Some(a));
}

#[test]
fn window_closed_while_open_relayouts_and_moves_selection() {
    let (mut f, id) = setup();
    let surfaces: Vec<_> = (0..3)
        .map(|i| f.map_window(id, (300, 200), RED, Some(&format!("w{i}"))))
        .collect();
    f.hecate().open_overview();
    let order = entries(&mut f);
    assert_eq!(order.len(), 3);
    // the middle one in reading order
    let b = order[1].clone();
    f.hecate().hecate.overview.selected = Some(b.clone());
    let index = f.windows().iter().position(|w| *w == b).unwrap();
    let surface = surfaces[index].clone();
    f.client(id).window(&surface).xdg_toplevel.destroy();
    f.double_roundtrip(id);
    let now = entries(&mut f);
    assert_eq!(now.len(), 2);
    assert!(!now.contains(&b), "no entry holds the closed window");
    assert_eq!(
        f.hecate().hecate.overview.selected,
        Some(order[2].clone()),
        "the next one"
    );
}

#[test]
fn window_mapped_while_open_gets_a_slot() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.map_window(id, (300, 200), BLUE, Some("b"));
    f.hecate().open_overview();
    f.map_window(id, (300, 200), RED, Some("c"));
    assert_eq!(entries(&mut f).len(), 3);
}

#[test]
fn locking_closes_the_overview() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.hecate().open_overview();
    assert!(f.hecate().hecate.overview.open);
    f.client(id).lock_session();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    assert!(f.hecate().hecate.wm.is_locked());
    let o = &f.hecate().hecate.overview;
    assert!(!o.open && o.anim.is_none(), "gone at once");
    f.hecate().toggle_overview();
    assert!(!f.hecate().hecate.overview.open, "nothing opens while locked");
}

#[test]
fn switching_workspace_while_open_shows_its_windows() {
    let (mut f, id) = setup();
    f.map_window(id, (300, 200), RED, Some("a"));
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    f.map_window(id, (300, 200), BLUE, Some("b"));
    f.map_window(id, (300, 200), BLUE, Some("c"));
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(1)), None);
    f.hecate().open_overview();
    assert_eq!(entries(&mut f).len(), 1);
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    f.dispatch();
    assert!(f.hecate().hecate.overview.open);
    assert_eq!(entries(&mut f).len(), 2);
}

#[test]
fn filter_matches_app_name_app_id_and_title() {
    assert!(filter_matches("fire", "org.mozilla.firefox", "Mozilla"));
    assert!(filter_matches("MOZ", "x", "Mozilla Firefox"));
    assert!(!filter_matches("kitty", "foot", "~"));
    assert!(filter_matches("", "foot", "~"));
}

// ---------------------------------------------------------------- drawing

mod drawing {
    use smithay::utils::{Logical, Rectangle};

    use super::super::animations::with_toml;
    use super::super::client::ClientId;
    use super::super::fixture::{Fixture, Frame};
    use super::{BLUE, RED};
    use crate::overview::Entry;

    pub(super) const BG: [u8; 3] = [0x10, 0x20, 0x30];
    /// A 6 px cyan single border, square, and a yellow gold tint.
    pub(super) const LOOK: &str = "[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 6\nradius = 0\n\
                                   [border.color]\nactive = \"#00ffff\"\ninactive = \"#00ffff\"\n\
                                   [border.tint]\ngold = \"#ffff00\"\n";

    /// A window backed by an shm buffer of `size`, with an app id.
    pub(super) fn window(
        f: &mut Fixture,
        id: ClientId,
        size: (i32, i32),
        rgba: [u8; 4],
        app_id: &str,
    ) -> wayland_client::protocol::wl_surface::WlSurface {
        let window = f.client(id).create_window();
        window.set_app_id(app_id);
        let surface = window.surface.clone();
        window.commit();
        f.double_roundtrip(id);
        let window = f.client(id).window(&surface);
        window.attach_shm(size.0, size.1, rgba);
        window.ack_last_and_commit();
        f.double_roundtrip(id);
        surface
    }

    /// The overview look plus `extra`, with a red and a blue 300×200 window.
    pub(super) fn two(extra: &str) -> Option<(Fixture, ClientId)> {
        let (mut f, id) = with_toml(&format!("{LOOK}{extra}"))?;
        window(&mut f, id, (300, 200), RED, "red");
        window(&mut f, id, (300, 200), BLUE, "blue");
        Some((f, id))
    }

    pub(super) fn entry(
        f: &mut Fixture,
        app_id: &str,
    ) -> (
        smithay::desktop::Window,
        crate::overview::layout::Slot,
        Rectangle<i32, Logical>,
    ) {
        use crate::wm::WindowExt;
        let o = &f.hecate().hecate.overview;
        let e: &Entry = o.per_output["headless-1"]
            .iter()
            .find(|e| e.window.app_id() == app_id)
            .unwrap();
        (e.window.clone(), e.slot, e.from)
    }

    pub(super) fn centre(r: Rectangle<i32, Logical>) -> (i32, i32) {
        (r.loc.x + r.size.w / 2, r.loc.y + r.size.h / 2)
    }

    pub(super) fn rgb(frame: &Frame, (x, y): (i32, i32)) -> [u8; 3] {
        let p = frame.pixel(x, y);
        [p[0], p[1], p[2]]
    }

    #[test]
    fn open_overview_draws_each_window_in_its_slot() {
        let Some((mut f, _)) = two("") else { return };
        let before = {
            let w = f.windows()[0].clone();
            f.hecate().hecate.wm.window_rect(&w)
        };
        f.hecate().open_overview();
        let frame = f.render("headless-1");
        let (_, red, _) = entry(&mut f, "red");
        let (_, blue, _) = entry(&mut f, "blue");
        let (x, y) = centre(red.content);
        frame.assert_rgb(x, y, [255, 0, 0], "red in its slot");
        let (x, y) = centre(blue.content);
        frame.assert_rgb(x, y, [0, 0, 255], "blue in its slot");
        // Somewhere the red window used to be that no slot covers.
        let spot = (before.loc.x + 2, before.loc.y + 2);
        assert!(![red.frame, blue.frame].iter().any(|r| r.contains(spot)));
        frame.assert_rgb(spot.0, spot.1, BG, "the old place is empty");
    }

    #[test]
    fn selected_slot_frame_is_gold() {
        let Some((mut f, _)) = two("") else { return };
        f.hecate().open_overview();
        let (red_w, red, _) = entry(&mut f, "red");
        let (_, blue, _) = entry(&mut f, "blue");
        f.hecate().hecate.overview.selected = Some(red_w);
        let frame = f.render("headless-1");
        frame.assert_rgb(
            centre(red.frame).0,
            red.frame.loc.y + 1,
            [255, 255, 0],
            "selected: gold",
        );
        frame.assert_rgb(
            centre(blue.frame).0,
            blue.frame.loc.y + 1,
            [0, 255, 255],
            "the other: inactive",
        );
    }

    #[test]
    fn non_matching_windows_are_dimmed() {
        let Some((mut f, _)) = two("") else { return };
        f.hecate().open_overview();
        f.hecate().hecate.overview.filter = "blue".into();
        let frame = f.render("headless-1");
        let (_, red, _) = entry(&mut f, "red");
        let (_, blue, _) = entry(&mut f, "blue");
        // 35 % red over the background.
        frame.assert_rgb(
            centre(red.content).0,
            centre(red.content).1,
            [100, 21, 31],
            "dimmed",
        );
        frame.assert_rgb(
            centre(blue.content).0,
            centre(blue.content).1,
            [0, 0, 255],
            "matching: as it is",
        );
    }

    #[test]
    fn slide_is_halfway_at_half_time() {
        let Some((mut f, _)) =
            two("[animations.overview]\nstyle = \"slide\"\nms = 400\ncurve = \"linear\"\n")
        else {
            return;
        };
        f.hecate().open_overview();
        f.advance_clock(200);
        let frame = f.render("headless-1");
        let (_, slot, from) = entry(&mut f, "red");
        let left = ((from.loc.x + slot.content.loc.x) as f64 / 2.).round() as i32;
        let top = ((from.loc.y + slot.content.loc.y) as f64 / 2.).round() as i32;
        let y = top + 10;
        frame.assert_rgb(left + 1, y, [255, 0, 0], "content starts at the halfway edge");
        assert_ne!(rgb(&frame, (left - 2, y)), [255, 0, 0], "and not before it");
    }

    #[test]
    fn pixelate_open_starts_in_blocks() {
        let Some((mut f, _)) = two(
            "[animations.overview]\nstyle = \"pixelate\"\nms = 400\ncurve = \"stepped\"\ncell = 16\nfade = false\n",
        ) else {
            return;
        };
        f.hecate().open_overview();
        let frame = f.render("headless-1");
        let (_, slot, _) = entry(&mut f, "red");
        // Blocks are counted from the middle of the framed window.
        let fr = slot.frame;
        let mid = (fr.size.w as f64 / 2., fr.size.h as f64 / 2.);
        let local = (slot.content.loc.x - fr.loc.x, slot.content.loc.y - fr.loc.y);
        let corner = |p: i32, m: f64| ((p as f64 - m) / 16.).floor() * 16. + m;
        let (cx, cy) = (corner(local.0, mid.0), corner(local.1, mid.1));
        let x0 = (cx.ceil() as i32 + 1).max(0) + fr.loc.x;
        let y0 = (cy.ceil() as i32 + 1).max(0) + fr.loc.y;
        let (x1, y1) = (x0 + 13, y0 + 13);
        let first = rgb(&frame, (x0, y0));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = rgb(&frame, (x, y));
                assert!(
                    p.iter()
                        .zip(first)
                        .all(|(a, b)| (*a as i32 - b as i32).abs() <= 6),
                    "block is uniform: {p:?} at ({x}, {y}) vs {first:?}"
                );
            }
        }
        f.advance_clock(400);
        let frame = f.render("headless-1");
        frame.assert_rgb(
            slot.content.loc.x,
            slot.content.loc.y,
            [255, 0, 0],
            "sharp at the end",
        );
        frame.assert_rgb(
            slot.content.loc.x - 1,
            slot.content.loc.y + 20,
            [0, 255, 255],
            "and its frame too",
        );
    }

    #[test]
    fn closing_returns_windows_to_their_places() {
        let Some((mut f, _)) = two("") else { return };
        // Blue maps last and is on top.
        let w = f.windows()[1].clone();
        let rect = f.hecate().hecate.wm.window_rect(&w);
        f.hecate().open_overview();
        f.render("headless-1");
        f.hecate().close_overview(None);
        f.dispatch();
        let (x, y) = centre(rect);
        f.render("headless-1")
            .assert_rgb(x, y, [0, 0, 255], "blue is back where it was");
        assert!(
            f.hecate().hecate.overview.per_output.is_empty(),
            "nothing left of the overview"
        );
    }

    #[test]
    fn fullscreen_window_gets_a_slot_and_the_bar_shows() {
        use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
        use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
        let Some((mut f, id)) = with_toml(LOOK) else {
            return;
        };
        let layer = f.client(id).create_layer(None, Layer::Top, "bar");
        let bar = layer.surface.clone();
        layer.set_configure_props(super::super::client::LayerConfigureProps {
            size: Some((0, 20)),
            anchor: Some(Anchor::Top | Anchor::Left | Anchor::Right),
            exclusive_zone: Some(20),
            ..Default::default()
        });
        layer.commit();
        f.double_roundtrip(id);
        let layer = f.client(id).layer(&bar);
        let (w, h) = layer.configures_received.last().unwrap().1.size;
        layer.attach_color([0, 255, 0, 255]);
        layer.set_size(w as u16, h as u16);
        layer.ack_last_and_commit();
        f.double_roundtrip(id);

        let surface = window(&mut f, id, (300, 200), RED, "red");
        f.client(id).window(&surface).set_fullscreen(None);
        f.client(id).window(&surface).commit();
        f.double_roundtrip(id);
        f.ack_and_resize(id, &surface, RED);
        let w = f.windows()[0].clone();
        assert!(crate::wm::WindowExt::data(&w).fullscreen, "fullscreen");
        f.render("headless-1")
            .assert_rgb(200, 10, [255, 0, 0], "fullscreen covers the bar");

        f.hecate().open_overview();
        let frame = f.render("headless-1");
        frame.assert_rgb(200, 10, [0, 255, 0], "the bar shows over the overview");
        let (_, slot, _) = entry(&mut f, "red");
        assert!(
            slot.frame.loc.y >= 20,
            "the slot is below the bar: {:?}",
            slot.frame
        );
        let (x, y) = centre(slot.content);
        frame.assert_rgb(x, y, [255, 0, 0], "the fullscreen window in its slot");
    }
}
