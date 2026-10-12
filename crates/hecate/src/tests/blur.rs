//! Blur behind layer surfaces, read back from software-rendered frames.
//!
//! The scene on a 400×300 output: a red wallpaper strip over the top 50 px of a green
//! background, and transparent "panel" layers asking for blur. Blur shows up as red and green
//! bleeding across y = 50; without it the edge stays sharp.

use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
use wayland_client::protocol::wl_surface::WlSurface;

use super::client::{ClientId, LayerConfigureProps};
use super::fixture::{Fixture, Frame};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

fn fixture(rules: &str) -> Option<(Fixture, ClientId)> {
    fixture_with_border("", rules)
}

fn fixture_with_border(border: &str, rules: &str) -> Option<(Fixture, ClientId)> {
    let toml = format!(
        "[general]\nbackground_color = \"#00ff00\"\n[border]\nwidth = 0\n{border}\n[animations]\nenable = false\n\
         [blur]\npasses = 2\nradius = 3\nnoise = 0\n{rules}"
    );
    let mut f = Fixture::with_toml(&toml);
    if !f.enable_rendering() {
        return None;
    }
    f.add_output("headless-1", (400, 300));
    let id = f.add_client();
    Some((f, id))
}

fn layer(
    f: &mut Fixture,
    id: ClientId,
    layer: Layer,
    ns: &str,
    anchor: Anchor,
    height: u32,
    rgba: [u8; 4],
) -> WlSurface {
    let l = f.client(id).create_layer(None, layer, ns);
    let surface = l.surface.clone();
    l.set_configure_props(LayerConfigureProps {
        size: Some((0, height)),
        anchor: Some(anchor | Anchor::Left | Anchor::Right),
        ..Default::default()
    });
    l.commit();
    f.double_roundtrip(id);
    let l = f.client(id).layer(&surface);
    let (w, h) = l.configures_received.last().unwrap().1.size;
    l.attach_color(rgba);
    l.set_size(w as u16, h as u16);
    l.ack_last_and_commit();
    f.double_roundtrip(id);
    surface
}

/// Red strip over y = 0..50.
fn wallpaper(f: &mut Fixture, id: ClientId) {
    layer(f, id, Layer::Background, "wallpaper", Anchor::Top, 50, RED);
}

fn rgb(frame: &Frame, x: i32, y: i32) -> [i32; 3] {
    let p = frame.pixel(x, y);
    [p[0] as i32, p[1] as i32, p[2] as i32]
}

const PANEL_RULE: &str = "[[layer_rules]]\nmatch = { namespace = \"^panel$\" }\nblur = true\n";

#[test]
fn without_blur_the_edge_is_sharp() {
    let Some((mut f, id)) = fixture("") else { return };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 48, [255, 0, 0], "red just above the edge");
    frame.assert_rgb(200, 52, [0, 255, 0], "green just below");
}

#[test]
fn blur_bleeds_across_the_edge_the_right_way_up() {
    let Some((mut f, id)) = fixture(PANEL_RULE) else {
        return;
    };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    let frame = f.render("headless-1");
    let at_edge = rgb(&frame, 200, 49);
    assert!(
        at_edge[0] > 40 && at_edge[1] > 40,
        "red and green mix at the edge: {at_edge:?}"
    );
    // Upside down, the panel would show the green from the bottom of the output.
    let top = rgb(&frame, 200, 5);
    assert!(top[0] > 2 * top[1], "mostly red near the top: {top:?}");
    let low = rgb(&frame, 200, 75);
    assert!(low[1] > 2 * low[0], "mostly green lower down: {low:?}");
    // Outside the panel nothing is blurred.
    frame.assert_rgb(200, 100, [0, 255, 0], "below the panel");
}

#[test]
fn xray_sees_through_windows() {
    // A blue window at (150, 100)..(250, 200) under a panel along the bottom (150..300).
    for (xray, expect_blue) in [(false, true), (true, false)] {
        let rules = format!("{PANEL_RULE}xray = {xray}\n");
        let Some((mut f, id)) = fixture(&rules) else {
            return;
        };
        f.map_shm_window(id, (100, 100), BLUE);
        layer(&mut f, id, Layer::Top, "panel", Anchor::Bottom, 150, CLEAR);
        let frame = f.render("headless-1");
        let p = rgb(&frame, 200, 180);
        if expect_blue {
            assert!(p[2] > 150, "without xray the window is blurred in: {p:?}");
        } else {
            assert!(p[2] < 30 && p[1] > 200, "xray shows only the wallpaper: {p:?}");
        }
        // The window outside the panel is untouched either way.
        frame.assert_rgb(200, 120, [0, 0, 255], "window above the panel");
    }
}

#[test]
fn ignore_alpha_keeps_blur_off_transparent_pixels() {
    let rules = format!("{PANEL_RULE}ignore_alpha = 0.5\n");
    let Some((mut f, id)) = fixture(&rules) else {
        return;
    };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 48, [255, 0, 0], "fully transparent: no blur");
    frame.assert_rgb(200, 52, [0, 255, 0], "fully transparent: no blur");

    // An opaque-enough panel gets blur behind it (and draws over it: half-transparent black).
    let Some((mut f, id)) = fixture(&rules) else {
        return;
    };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, [0, 0, 0, 200]);
    let frame = f.render("headless-1");
    let p = rgb(&frame, 200, 49);
    assert!(p[0] > 5 && p[1] > 5, "blur behind the panel: {p:?}");
}

#[test]
fn blurred_scene_is_cached_until_it_changes() {
    let Some((mut f, id)) = fixture(PANEL_RULE) else {
        return;
    };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    let first = f.render("headless-1");
    let second = f.render("headless-1");
    assert_eq!(rgb(&first, 200, 49), rgb(&second, 200, 49));
    let o = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    let entries = |o: &smithay::output::Output| {
        o.user_data()
            .get::<std::cell::RefCell<crate::render::blur::BlurCache>>()
            .unwrap()
            .borrow()
            .len()
    };
    assert_eq!(entries(&o), 1, "one blurred scene, reused");

    // A second blurred panel over the same wallpaper with xray shares nothing with the first
    // (the first panel is part of its scene), so there are two.
    let rules = format!("{PANEL_RULE}[[layer_rules]]\nmatch = {{ namespace = \"^dock$\" }}\nblur = true\n");
    let Some((mut f, id)) = fixture(&rules) else {
        return;
    };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    layer(&mut f, id, Layer::Top, "dock", Anchor::Bottom, 40, CLEAR);
    f.render("headless-1");
    let o = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    assert_eq!(entries(&o), 2);
}

#[test]
fn blur_disabled_globally() {
    let Some((mut f, id)) = fixture(PANEL_RULE) else {
        return;
    };
    f.hecate().hecate.config.blur.enable = false;
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 48, [255, 0, 0], "sharp edge");
}

#[test]
fn ignore_alpha_masks_by_the_buffer_contents() {
    // Same as above, with real shm buffers (the masked shader path).
    let rules = format!("{PANEL_RULE}ignore_alpha = 0.5\n");
    for (rgba, blurred) in [([0, 0, 0, 0], false), ([0, 0, 0, 200], true)] {
        let Some((mut f, id)) = fixture(&rules) else {
            return;
        };
        wallpaper(&mut f, id);
        let surface = layer(&mut f, id, Layer::Top, "panel", Anchor::Top, 80, CLEAR);
        let l = f.client(id).layer(&surface);
        l.attach_shm(400, 80, rgba);
        l.commit();
        f.double_roundtrip(id);
        let frame = f.render("headless-1");
        let p = rgb(&frame, 200, 49);
        if blurred {
            assert!(p[0] > 5 && p[1] > 5, "opaque enough: blur behind: {p:?}");
        } else {
            frame.assert_rgb(200, 48, [255, 0, 0], "transparent buffer: no blur");
            frame.assert_rgb(200, 52, [0, 255, 0], "transparent buffer: no blur");
        }
    }
}

// ---------------------------------------------------------------- ext-background-effect

fn region_fixture(extra: &str) -> Option<(Fixture, ClientId, WlSurface)> {
    let (mut f, id) = fixture_with_border("radius = 20\nclip_content = true", extra)?;
    // Red over x = 0..155: just right of the transparent window's left edge (150).
    let l = f.client(id).create_layer(None, Layer::Background, "wallpaper");
    let surface = l.surface.clone();
    l.set_configure_props(LayerConfigureProps {
        size: Some((155, 0)),
        anchor: Some(Anchor::Left | Anchor::Top | Anchor::Bottom),
        ..Default::default()
    });
    l.commit();
    f.double_roundtrip(id);
    let l = f.client(id).layer(&surface);
    let (w, h) = l.configures_received.last().unwrap().1.size;
    l.attach_color(RED);
    l.set_size(w as u16, h as u16);
    l.ack_last_and_commit();
    f.double_roundtrip(id);
    let window = f.map_shm_window(id, (100, 100), CLEAR);
    Some((f, id, window))
}

#[test]
fn background_effect_advertises_blur() {
    let Some((mut f, id)) = fixture("") else { return };
    f.roundtrip(id);
    assert_eq!(f.client(id).state.background_effect_caps, Some(1));
    let mut config = f.hecate().hecate.config.clone();
    config.blur.enable = false;
    f.hecate().apply_config(config);
    f.roundtrip(id);
    assert_eq!(
        f.client(id).state.background_effect_caps,
        Some(0),
        "capabilities resent when blur is switched off"
    );
}

#[test]
fn window_blur_region_blurs_behind_it_inside_its_corners() {
    let Some((mut f, id, window)) = region_fixture("") else {
        return;
    };
    let frame = f.render("headless-1");
    frame.assert_rgb(152, 150, [255, 0, 0], "no region yet: sharp");

    f.client(id)
        .state
        .set_blur_region(&window, Some(&[(0, 0, 100, 100)]));
    f.client(id).window(&window).commit();
    f.double_roundtrip(id);
    let frame = f.render("headless-1");
    let p = rgb(&frame, 152, 150);
    assert!(
        p[1] > 20 && p[0] > 20,
        "blurred across x = 155 behind the window: {p:?}"
    );
    // The rounded corner is outside the window: the wallpaper shows through unblurred.
    frame.assert_rgb(151, 101, [255, 0, 0], "corner left alone");
    // Outside the window, nothing changes.
    frame.assert_rgb(148, 150, [255, 0, 0], "left of the window");
}

#[test]
fn blur_region_is_clipped_to_its_rectangles_and_can_be_cleared() {
    let Some((mut f, id, window)) = region_fixture("") else {
        return;
    };
    // Only the bottom half of the window.
    let effect = f
        .client(id)
        .state
        .set_blur_region(&window, Some(&[(0, 50, 100, 50)]));
    f.client(id).window(&window).commit();
    f.double_roundtrip(id);
    let frame = f.render("headless-1");
    frame.assert_rgb(152, 120, [255, 0, 0], "top half: not in the region");
    let p = rgb(&frame, 152, 170);
    assert!(p[1] > 20, "bottom half blurred: {p:?}");

    effect.set_blur_region(None);
    f.client(id).window(&window).commit();
    f.double_roundtrip(id);
    f.render("headless-1")
        .assert_rgb(152, 170, [255, 0, 0], "cleared");
}

#[test]
fn locked_screen_shows_only_layers_that_ask() {
    let rule = "[[layer_rules]]\nmatch = { namespace = \"^wallpaper$\" }\nshow_when_locked = true\n";
    let Some((mut f, id)) = fixture(rule) else { return };
    wallpaper(&mut f, id);
    layer(&mut f, id, Layer::Top, "panel", Anchor::Bottom, 50, BLUE);
    f.map_window(id, (100, 100), BLUE, None);
    f.hecate().hecate.wm.lock = crate::wm::LockState::Locked;
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 20, [255, 0, 0], "wallpaper under the lock");
    frame.assert_rgb(200, 280, [0, 0, 0], "panel hidden by the lock");
    frame.assert_rgb(200, 150, [0, 0, 0], "window hidden by the lock");
}
