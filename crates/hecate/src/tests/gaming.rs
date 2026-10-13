//! Gaming policy: VRR on fullscreen, tearing requests (rule, tearing-control, content type).

use smithay::reexports::wayland_protocols::wp::content_type::v1::client::wp_content_type_v1;
use smithay::reexports::wayland_protocols::wp::tearing_control::v1::client::wp_tearing_control_v1::PresentationHint;

use super::fixture::Fixture;
use crate::config::action::Action;

const RED: [u8; 4] = [255, 0, 0, 255];

fn vrr(f: &mut Fixture) -> Option<bool> {
    f.dispatch();
    f.hecate().backend.headless().vrr.get("headless-1").copied()
}

#[test]
fn vrr_follows_fullscreen_when_set_to_fullscreen() {
    let mut f = Fixture::with_toml("[outputs.headless-1]\nadaptive_sync = \"fullscreen\"");
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let s = f.map_window(id, (400, 300), RED, Some("game"));
    assert_eq!(vrr(&mut f), Some(false));

    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &s, RED);
    assert_eq!(vrr(&mut f), Some(true), "VRR on with a fullscreen window");
    let g = f.hecate().ipc_gaming();
    assert!(g.outputs[0].vrr_enabled);

    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &s, RED);
    assert_eq!(vrr(&mut f), Some(false), "off again");
}

#[test]
fn vrr_on_and_off_follow_config() {
    let mut f = Fixture::with_toml("[outputs.headless-1]\nadaptive_sync = \"on\"");
    f.add_output("headless-1", (1920, 1080));
    assert_eq!(vrr(&mut f), Some(true));
    f.hecate()
        .set_override("outputs.headless-1.adaptive_sync", serde_json::json!("off"))
        .unwrap();
    assert_eq!(vrr(&mut f), Some(false));
}

fn fullscreen_game(
    f: &mut Fixture,
    toml: &str,
) -> (
    super::client::ClientId,
    wayland_client::protocol::wl_surface::WlSurface,
) {
    *f = Fixture::with_toml(toml);
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let s = f.map_window(id, (400, 300), RED, Some("game"));
    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &s, RED);
    (id, s)
}

#[test]
fn tearing_needs_a_request_and_the_policy() {
    let mut f = Fixture::new();
    let (id, s) = fullscreen_game(&mut f, "");
    assert!(
        !f.hecate().ipc_gaming().outputs[0].tearing_allowed,
        "nobody asked"
    );

    // The game asks through wp-tearing-control; it applies on commit.
    let tc = {
        let st = &f.client(id).state;
        st.tearing_manager
            .as_ref()
            .unwrap()
            .get_tearing_control(&s, &st.qh, ())
    };
    tc.set_presentation_hint(PresentationHint::Async);
    f.double_roundtrip(id);
    assert!(
        !f.hecate().ipc_gaming().outputs[0].tearing_allowed,
        "not before commit"
    );
    f.client(id).window(&s).commit();
    f.double_roundtrip(id);
    let g = f.hecate().ipc_gaming();
    assert!(g.outputs[0].tearing_allowed);
    assert!(g.windows[0].tearing_requested);

    // Not when it isn't fullscreen (default policy), and never with allow_tearing = never.
    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &s, RED);
    assert!(!f.hecate().ipc_gaming().outputs[0].tearing_allowed);
    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &s, RED);
    f.hecate()
        .set_override("gaming.allow_tearing", serde_json::json!("never"))
        .unwrap();
    assert!(!f.hecate().ipc_gaming().outputs[0].tearing_allowed);
}

#[test]
fn content_type_game_and_window_rule_count_as_tearing_requests() {
    let mut f = Fixture::new();
    let (id, s) = fullscreen_game(&mut f, "");
    let ct = {
        let st = &f.client(id).state;
        st.content_type_manager
            .as_ref()
            .unwrap()
            .get_surface_content_type(&s, &st.qh, ())
    };
    ct.set_content_type(wp_content_type_v1::Type::Game);
    f.client(id).window(&s).commit();
    f.double_roundtrip(id);
    let g = f.hecate().ipc_gaming();
    assert_eq!(g.windows[0].content_type.as_deref(), Some("game"));
    assert!(g.outputs[0].tearing_allowed);

    let mut f = Fixture::new();
    fullscreen_game(
        &mut f,
        "[[window_rules]]\nmatch = { app_id = \"game\" }\ntearing = true",
    );
    assert!(f.hecate().ipc_gaming().outputs[0].tearing_allowed, "window rule");
}
