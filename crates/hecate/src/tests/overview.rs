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
