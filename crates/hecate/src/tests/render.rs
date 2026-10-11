//! Pixel tests on a software-rendered headless output (400×300).
//!
//! A 100×100 window is centred at (150, 100)..(250, 200).

use super::fixture::Fixture;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BG: [u8; 3] = [0x10, 0x20, 0x30];
const ACTIVE: [u8; 3] = [0x00, 0xff, 0xff];
const INACTIVE: [u8; 3] = [0x80, 0x80, 0x80];

fn fixture(extra: &str) -> Option<(Fixture, super::client::ClientId)> {
    let toml = format!(
        "[general]\nbackground_color = \"#102030\"\n[border.color]\nactive = \"#00ffff\"\ninactive = \"#808080\"\n{extra}"
    );
    let mut f = Fixture::with_toml(&toml);
    if !f.enable_rendering() {
        return None;
    }
    f.add_output("headless-1", (400, 300));
    let id = f.add_client();
    Some((f, id))
}

#[test]
fn background_colour() {
    let Some((mut f, _)) = fixture("") else { return };
    let frame = f.render("headless-1");
    frame.assert_rgb(5, 5, BG, "background");
    frame.assert_rgb(395, 295, BG, "background");
}

#[test]
fn window_content_and_square_border() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 4\nradius = 0") else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 150, [255, 0, 0], "window centre");
    frame.assert_rgb(151, 101, [255, 0, 0], "inside the top-left corner");
    frame.assert_rgb(148, 150, ACTIVE, "left border");
    frame.assert_rgb(251, 150, ACTIVE, "right border");
    frame.assert_rgb(200, 97, ACTIVE, "top border");
    frame.assert_rgb(146, 96, ACTIVE, "square outer corner is border");
    frame.assert_rgb(144, 150, BG, "outside the border");
}

#[test]
fn rounded_corners_clip_content() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 4\nradius = 20\nclip_content = true") else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 150, [255, 0, 0], "centre");
    // The very corner of the geometry is outside both concentric arcs (r = 20 and r = 24).
    frame.assert_rgb(151, 101, BG, "content corner clipped away");
    // Between the arcs (distance ~21 from the arc centre at (170, 120)): border.
    frame.assert_rgb(155, 105, ACTIVE, "border follows the rounded corner");
    // Outer corner of the border is rounded away: background.
    frame.assert_rgb(147, 97, BG, "rounded outer corner");
    // Straight edges are still border.
    frame.assert_rgb(148, 150, ACTIVE, "left edge");
}

#[test]
fn without_clipping_content_covers_the_corner() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 4\nradius = 20\nclip_content = false") else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(151, 101, [255, 0, 0], "unclipped content corner");
}

#[test]
fn per_corner_radii() {
    let extra = "[border]\nwidth = 4\nclip_content = true\n[border.radius]\ntop_left = 30\nbottom_right = 0";
    let Some((mut f, id)) = fixture(extra) else { return };
    f.map_shm_window(id, (100, 100), RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(153, 103, BG, "rounded top-left is clipped");
    frame.assert_rgb(157, 107, ACTIVE, "border between the r=30 and r=34 arcs");
    frame.assert_rgb(248, 198, [255, 0, 0], "square bottom-right keeps content");
    frame.assert_rgb(249, 101, [255, 0, 0], "top-right radius 0");
    frame.assert_rgb(251, 201, ACTIVE, "square bottom-right outer corner is border");
}

#[test]
fn zero_width_still_clips() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 0\nradius = 20\nclip_content = true") else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(151, 101, BG, "no border, corner clipped to background");
    frame.assert_rgb(200, 150, [255, 0, 0], "centre");
    frame.assert_rgb(148, 150, BG, "no border drawn");
}

#[test]
fn inactive_border_colour_and_stacking() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 4\nradius = 0\n[placement]\npolicy = \"center\"")
    else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.map_shm_window(id, (60, 60), GREEN);
    let frame = f.render("headless-1");
    // The green window is centred on top of the red one and focused.
    frame.assert_rgb(200, 150, [0, 255, 0], "focused window on top");
    frame.assert_rgb(148, 150, INACTIVE, "unfocused window border");
    frame.assert_rgb(168, 150, ACTIVE, "focused window border");
}

#[test]
fn maximized_window_has_no_border_or_rounding() {
    let Some((mut f, id)) = fixture("[border]\nwidth = 4\nradius = 20") else {
        return;
    };
    let s = f.map_shm_window(id, (100, 100), RED);
    f.hecate()
        .do_action(crate::config::action::Action::Maximize, None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &s, RED);
    let frame = f.render("headless-1");
    frame.assert_rgb(0, 0, [255, 0, 0], "square corner when maximized");
    frame.assert_rgb(399, 299, [255, 0, 0], "fills the output");
}
