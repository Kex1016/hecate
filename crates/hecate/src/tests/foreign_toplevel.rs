//! wlr-foreign-toplevel-management and ext-foreign-toplevel-list, as a taskbar would see them.

use smithay::reexports::wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::State as TlState;

use super::fixture::Fixture;
use crate::config::action::Action;
use crate::wm::{Maximized, WindowExt};

const RED: [u8; 4] = [255, 0, 0, 255];

#[test]
fn taskbar_sees_windows_titles_and_focus() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let bar = f.add_client();

    let sa = f.map_window(app, (400, 300), RED, Some("alpha"));
    f.client(app).window(&sa).set_title("Alpha one");
    f.client(app).window(&sa).commit();
    f.map_window(app, (400, 300), RED, Some("beta"));
    f.double_roundtrip(bar);

    let st = &f.client(bar).state;
    assert!(st.wlr_toplevel_manager.is_some(), "wlr manager advertised");
    assert!(st.ext_toplevel_list.is_some(), "ext list advertised");
    let (_, a) = st.wlr_toplevel("alpha").expect("alpha listed").clone();
    let (_, b) = st.wlr_toplevel("beta").expect("beta listed").clone();
    assert_eq!(a.title, "Alpha one");
    assert!(!a.activated(), "unfocused window not activated");
    assert!(b.activated(), "focused window activated");
    assert_eq!(a.outputs, 1, "output_enter sent");

    let (_, ea) = st.ext_toplevel("alpha").expect("alpha in ext list").clone();
    let (_, eb) = st.ext_toplevel("beta").expect("beta in ext list").clone();
    assert!(ea.identifier.starts_with("hecate-"));
    assert_ne!(ea.identifier, eb.identifier, "identifiers unique");

    // Focus moves -> activated moves.
    f.hecate().do_action(Action::FocusNext, None);
    f.double_roundtrip(bar);
    let st = &f.client(bar).state;
    assert!(st.wlr_toplevel("alpha").unwrap().1.activated());
    assert!(!st.wlr_toplevel("beta").unwrap().1.activated());
}

#[test]
fn closed_windows_are_reported_and_late_binders_see_existing() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let s = f.map_window(app, (400, 300), RED, Some("gone"));

    // A client connecting after the window mapped still gets it.
    let bar = f.add_client();
    f.double_roundtrip(bar);
    assert!(f.client(bar).state.wlr_toplevel("gone").is_some());

    let w = f.client(app).window(&s);
    w.attach_null();
    w.commit();
    f.double_roundtrip(app);
    f.double_roundtrip(bar);
    let st = &f.client(bar).state;
    assert!(st.wlr_toplevels.iter().all(|(_, t)| t.closed), "closed sent");
    assert!(st.ext_toplevels.iter().all(|(_, t)| t.closed), "ext closed sent");
}

#[test]
fn taskbar_requests_control_windows() {
    let mut f = Fixture::with_toml("[border]\nkeep_on_maximize = false");
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let bar = f.add_client();
    f.map_window(app, (400, 300), RED, Some("a"));
    f.map_window(app, (400, 300), RED, Some("b"));
    f.double_roundtrip(bar);
    let (a_handle, _) = f.client(bar).state.wlr_toplevel("a").unwrap().clone();
    let seat = f.client(bar).state.seats.keys().next().unwrap().clone();
    let wa = f.windows()[0].clone();

    // activate
    a_handle.activate(&seat);
    f.double_roundtrip(bar);
    assert_eq!(f.focused(), Some(wa.clone()), "activate focuses");

    // maximize
    a_handle.set_maximized();
    f.double_roundtrip(bar);
    assert_eq!(wa.data().maximized, Maximized::Both);
    f.double_roundtrip(bar);
    assert!(
        f.client(bar)
            .state
            .wlr_toplevel("a")
            .unwrap()
            .1
            .has_state(TlState::Maximized)
    );
    a_handle.unset_maximized();
    f.double_roundtrip(bar);
    assert_eq!(wa.data().maximized, Maximized::No);

    // minimize
    a_handle.set_minimized();
    f.double_roundtrip(bar);
    f.double_roundtrip(bar);
    assert!(wa.data().minimized);
    let t = f.client(bar).state.wlr_toplevel("a").unwrap().1.clone();
    assert!(t.has_state(TlState::Minimized));
    assert!(!t.activated(), "minimized window loses focus");
    a_handle.unset_minimized();
    f.double_roundtrip(bar);
    assert!(!wa.data().minimized);

    // fullscreen
    a_handle.set_fullscreen(None);
    f.double_roundtrip(bar);
    assert!(wa.data().fullscreen);
    a_handle.unset_fullscreen();
    f.double_roundtrip(bar);
    assert!(!wa.data().fullscreen);

    // close
    a_handle.close();
    f.double_roundtrip(bar);
    f.double_roundtrip(app);
    let app_window = f.client(app).state.windows.first().unwrap();
    assert!(app_window.close_requested, "close forwarded to the client");
}

#[test]
fn skip_taskbar_rule_hides_window() {
    let mut f = Fixture::with_toml("[[window_rules]]\nmatch = { app_id = \"hidden\" }\nskip_taskbar = true");
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let bar = f.add_client();
    f.map_window(app, (400, 300), RED, Some("hidden"));
    f.map_window(app, (400, 300), RED, Some("shown"));
    f.double_roundtrip(bar);
    let st = &f.client(bar).state;
    assert!(st.wlr_toplevel("hidden").is_none());
    assert!(st.wlr_toplevel("shown").is_some());
}
