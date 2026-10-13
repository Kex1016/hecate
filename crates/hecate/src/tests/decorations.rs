//! Decoration negotiation: server-side by default, client-side per config or window rule.

use smithay::reexports::wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode;

use super::fixture::Fixture;

fn negotiated(toml: &str, app_id: &str, request: Option<Mode>) -> Mode {
    let mut f = Fixture::with_toml(toml);
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let window = f.client(id).create_window();
    window.set_app_id(app_id);
    let surface = window.surface.clone();
    let deco = f.client(id).state.decorate(&surface, request);
    f.client(id).window(&surface).commit();
    f.double_roundtrip(id);
    *f.client(id)
        .state
        .decoration_modes
        .get(&deco)
        .expect("no decoration configure")
}

#[test]
fn server_side_by_default() {
    assert_eq!(negotiated("", "app", None), Mode::ServerSide);
}

#[test]
fn client_asking_for_csd_still_gets_server_side() {
    assert_eq!(negotiated("", "app", Some(Mode::ClientSide)), Mode::ServerSide);
}

#[test]
fn config_can_choose_client_side() {
    assert_eq!(
        negotiated("[general]\ndecorations = \"client\"", "app", None),
        Mode::ClientSide
    );
}

#[test]
fn window_rule_overrides_per_app() {
    let toml = "[[window_rules]]\nmatch = { app_id = \"keep-csd\" }\ndecorations = \"client\"";
    assert_eq!(negotiated(toml, "keep-csd", None), Mode::ClientSide);
    assert_eq!(negotiated(toml, "other", None), Mode::ServerSide);
}
