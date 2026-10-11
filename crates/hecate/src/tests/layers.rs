use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use super::client::LayerConfigureProps;
use super::fixture::Fixture;
use crate::config::action::Action;
use crate::wm::Wm;

const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

fn bar(
    f: &mut Fixture,
    id: super::client::ClientId,
    height: u32,
    ns: &str,
) -> wayland_client::protocol::wl_surface::WlSurface {
    let layer = f.client(id).create_layer(None, Layer::Top, ns);
    let surface = layer.surface.clone();
    layer.set_configure_props(LayerConfigureProps {
        size: Some((0, height)),
        anchor: Some(Anchor::Top | Anchor::Left | Anchor::Right),
        exclusive_zone: Some(height as i32),
        ..Default::default()
    });
    layer.commit();
    f.double_roundtrip(id);
    let layer = f.client(id).layer(&surface);
    let (w, h) = layer.configures_received.last().unwrap().1.size;
    layer.attach_color(BLUE);
    layer.set_size(w as u16, h as u16);
    layer.ack_last_and_commit();
    f.double_roundtrip(id);
    surface
}

#[test]
fn exclusive_zone_shrinks_usable_area() {
    let mut f = Fixture::new();
    let output = f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let surface = bar(&mut f, id, 30, "quickshell:bar");
    let cfg = f.client(id).layer(&surface).configures_received.last().unwrap().1;
    assert_eq!(cfg.size, (1920, 30), "full-width bar");
    let area = Wm::usable_area(&output, &f.hecate().hecate.config);
    assert_eq!(area.loc, (0, 30).into());
    assert_eq!(area.size, (1920, 1050).into());
}

#[test]
fn maximized_window_avoids_the_bar_and_follows_changes() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let win = f.map_window(id, (400, 300), RED, None);
    f.hecate().do_action(Action::Maximize, None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &win, RED);

    bar(&mut f, id, 40, "bar");
    let cfg = f
        .client(id)
        .window(&win)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone();
    assert_eq!(cfg.size, (1920, 1040), "re-maximized under the bar");
    f.ack_and_resize(id, &win, RED);
    let w = f.windows()[0].clone();
    assert_eq!(f.hecate().hecate.wm.window_rect(&w).loc, (0, 40).into());
}

#[test]
fn exclusive_keyboard_layer_takes_focus() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.map_window(id, (400, 300), RED, None);
    let layer = f.client(id).create_layer(None, Layer::Overlay, "launcher");
    let surface = layer.surface.clone();
    layer.set_configure_props(LayerConfigureProps {
        size: Some((600, 400)),
        kb_interactivity: Some(KeyboardInteractivity::Exclusive),
        ..Default::default()
    });
    layer.commit();
    f.double_roundtrip(id);
    let layer = f.client(id).layer(&surface);
    layer.attach_color(BLUE);
    layer.set_size(600, 400);
    layer.ack_last_and_commit();
    f.double_roundtrip(id);
    let focus = f.hecate().keyboard_focus();
    assert!(focus.is_some());
    assert!(
        f.focused().is_none(),
        "keyboard focus is on the layer, not the window"
    );
}

#[test]
fn layer_rules_resolve_in_order() {
    let toml = r#"
[[layer_rules]]
match = { namespace = "quickshell:.*" }
blur = true
ignore_alpha = 0.79

[[layer_rules]]
match = { namespace = "quickshell:background" }
blur = false
"#;
    let config = crate::config::load::from_toml_str(toml).unwrap();
    let bar = crate::render::layer_rules_for(&config, "quickshell:bar");
    assert_eq!(bar.blur, Some(true));
    assert_eq!(bar.ignore_alpha, Some(0.79));
    let bg = crate::render::layer_rules_for(&config, "quickshell:background");
    assert_eq!(bg.blur, Some(false));
    let other = crate::render::layer_rules_for(&config, "waybar");
    assert_eq!(other.blur, None);
}
