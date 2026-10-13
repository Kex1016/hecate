//! hyprland-focus-grab-v1 as QuickShell's popups use it.

use super::fixture::Fixture;
use crate::wm::WindowExt;

const RED: [u8; 4] = [255, 0, 0, 255];

fn setup() -> (Fixture, super::client::ClientId) {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    (f, id)
}

fn server_surface(
    f: &mut Fixture,
    idx: usize,
) -> smithay::reexports::wayland_server::protocol::wl_surface::WlSurface {
    f.windows()[idx].toplevel_surface().wl_surface().clone()
}

fn keyboard_focus(
    f: &mut Fixture,
) -> Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface> {
    f.hecate().hecate.seat.get_keyboard().unwrap().current_focus()
}

#[test]
fn grab_takes_keyboard_and_outside_press_clears() {
    let (mut f, id) = setup();
    let popup = f.map_window(id, (300, 200), RED, Some("popup"));
    f.map_window(id, (300, 200), RED, Some("other"));
    let (sa, sb) = (server_surface(&mut f, 0), server_surface(&mut f, 1));
    assert_eq!(
        keyboard_focus(&mut f).as_ref(),
        Some(&sb),
        "newest window focused"
    );

    let grab = f.client(id).state.focus_grab(&[&popup]);
    f.double_roundtrip(id);
    assert!(f.hecate().hecate.focus_grab_state.is_active());
    assert_eq!(
        keyboard_focus(&mut f).as_ref(),
        Some(&sa),
        "keyboard moved into the whitelist"
    );

    // A press inside keeps it.
    f.hecate().focus_grab_on_press(Some(&sa));
    f.double_roundtrip(id);
    assert!(f.client(id).state.focus_grabs_cleared.is_empty());

    // A press outside clears it and focus goes back to the windows.
    f.hecate().focus_grab_on_press(Some(&sb));
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.focus_grabs_cleared, vec![grab]);
    assert!(!f.hecate().hecate.focus_grab_state.is_active());
    assert_eq!(keyboard_focus(&mut f).as_ref(), Some(&sb));
}

#[test]
fn press_on_nothing_clears() {
    let (mut f, id) = setup();
    let popup = f.map_window(id, (300, 200), RED, Some("popup"));
    let grab = f.client(id).state.focus_grab(&[&popup]);
    f.double_roundtrip(id);
    f.hecate().focus_grab_on_press(None);
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.focus_grabs_cleared, vec![grab]);
}

#[test]
fn empty_commit_and_dead_surface_clear() {
    let (mut f, id) = setup();
    let popup = f.map_window(id, (300, 200), RED, Some("popup"));

    let grab = f.client(id).state.focus_grab(&[&popup]);
    f.double_roundtrip(id);
    grab.remove_surface(&popup);
    grab.commit();
    f.double_roundtrip(id);
    assert_eq!(
        f.client(id).state.focus_grabs_cleared,
        vec![grab.clone()],
        "inert after empty commit"
    );

    // A whitelisted surface going away counts as removing it.
    let grab2 = f.client(id).state.focus_grab(&[&popup]);
    f.double_roundtrip(id);
    assert!(f.hecate().hecate.focus_grab_state.is_active());
    let w = f.client(id).window(&popup);
    w.xdg_toplevel.destroy();
    w.xdg_surface.destroy();
    popup.destroy();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    assert!(
        f.client(id).state.focus_grabs_cleared.contains(&grab2),
        "cleared when the surface died"
    );
}

#[test]
fn new_grab_replaces_old() {
    let (mut f, id) = setup();
    let a = f.map_window(id, (300, 200), RED, Some("a"));
    let b = f.map_window(id, (300, 200), RED, Some("b"));
    let g1 = f.client(id).state.focus_grab(&[&a]);
    f.double_roundtrip(id);
    let _g2 = f.client(id).state.focus_grab(&[&b]);
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.focus_grabs_cleared, vec![g1]);
    let sb = server_surface(&mut f, 1);
    assert_eq!(keyboard_focus(&mut f).as_ref(), Some(&sb));
}
