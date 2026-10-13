//! ext-session-lock: where the keyboard goes while locked.

use smithay::wayland::session_lock::LockSurface;

use super::client::ClientId;
use super::fixture::Fixture;

/// Two side-by-side outputs, locked, with a lock surface on each.
fn locked() -> (Fixture, ClientId) {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    f.client(id).lock_session();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    assert!(f.client(id).state.session_locked);
    (f, id)
}

fn lock_surface_of(f: &mut Fixture, output: &str) -> LockSurface {
    let wm = &f.hecate().hecate.wm;
    let output = wm.output_by_name(output).cloned().unwrap();
    wm.lock_surface(&output).cloned().unwrap()
}

fn keyboard_focus(
    f: &mut Fixture,
) -> Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface> {
    f.hecate().hecate.seat.get_keyboard().unwrap().current_focus()
}

#[test]
fn locked_keyboard_moves_to_another_lock_surface_when_its_monitor_goes() {
    let (mut f, id) = locked();
    let focused = f.hecate().hecate.wm.focused_output.clone().unwrap();
    let other = if focused.name() == "headless-1" {
        "headless-2"
    } else {
        "headless-1"
    };
    assert_eq!(
        keyboard_focus(&mut f).as_ref(),
        Some(lock_surface_of(&mut f, &focused.name()).wl_surface()),
        "the focused monitor's lock surface has the keyboard"
    );

    // A monitor in deep sleep drops off the bus.
    f.hecate().hecate.remove_output(&focused);
    f.double_roundtrip(id);

    assert_eq!(
        keyboard_focus(&mut f).as_ref(),
        Some(lock_surface_of(&mut f, other).wl_surface()),
        "the remaining lock surface has the keyboard"
    );
}

#[test]
fn clicking_a_monitor_while_locked_gives_its_lock_surface_the_keyboard() {
    let (mut f, id) = locked();
    let focused = f.hecate().hecate.wm.focused_output.clone().unwrap();
    let other = if focused.name() == "headless-1" {
        "headless-2"
    } else {
        "headless-1"
    };
    let other_output = f.hecate().hecate.wm.output_by_name(other).cloned().unwrap();
    let geo = crate::wm::Wm::output_geometry(&other_output);
    let center = (geo.loc + geo.size.downscale(2).to_point()).to_f64();

    // What a button press does while locked.
    f.hecate().focus_lock_surface_at(center);
    f.double_roundtrip(id);

    assert_eq!(
        keyboard_focus(&mut f).as_ref(),
        Some(lock_surface_of(&mut f, other).wl_surface()),
        "the clicked monitor's lock surface has the keyboard"
    );
    assert_eq!(f.hecate().hecate.wm.focused_output.as_ref(), Some(&other_output));
}
