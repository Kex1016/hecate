//! The haiiro look: windows and layers that pixelate in and out, the double frame with its
//! notch label, border tints, and a splash that hands over without a fade.
//!
//! The output is 400×300. Windows are placed by their frames, so one whose frame is as wide
//! on every side maps centred.

use std::path::{Path, PathBuf};
use std::time::Duration;

use hecate_ipc::{Reply, Request, Selector};
use serde_json::{Value, json};
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
use smithay::utils::{Logical, Rectangle};
use wayland_client::protocol::wl_surface::WlSurface;

use super::animations::{bar, running, with_toml};
use super::client::ClientId;
use super::fixture::{Fixture, Frame};
use super::splash;
use crate::config::Color;
use crate::config::action::{Action, Edge, WorkspaceRef};
use crate::render::{HecateElement, WindowElement, label};
use crate::wm::WindowExt;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BG: [u8; 3] = [0x10, 0x20, 0x30];
const CYAN: [u8; 3] = [0, 255, 255];

/// A 6 px cyan border, to go around a 116×116 red window: a 128×128 frame at (136, 86), so
/// blocks of 16, 8, 4 and 2 pixels counted from its middle all start at its edges. Along
/// row 150 the leftmost block holds border on its left and content from 6 px in.
const BLOCKS: &str = "[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 6\nradius = 0\n\
                      [border.color]\nactive = \"#00ffff\"\ninactive = \"#00ffff\"\n";

/// `BLOCKS` with one animation set to pixelate from 16 px blocks in four steps of 100 ms.
fn pixelate(name: &str) -> String {
    format!(
        "{BLOCKS}[animations.{name}]\nstyle = \"pixelate\"\nms = 400\ncurve = \"stepped\"\ncell = 16\nfade = false\n"
    )
}

/// The left edge of the `BLOCKS` window on row 150, drawn in blocks of `cell` pixels (1:
/// sharp). A block is the average of what it covers.
fn assert_blocks(frame: &Frame, cell: i32) {
    let red = [255, 0, 0];
    frame.assert_rgb(135, 150, BG, "nothing outside the frame");
    match cell {
        16 => {
            frame.assert_rgb(136, 150, [159, 96, 96], "16 px block: 6 of border, 10 of content");
            frame.assert_rgb(151, 150, [159, 96, 96], "16 px block, far end");
            frame.assert_rgb(152, 150, red, "the next block is all content");
            frame.assert_rgb(
                140,
                90,
                [100, 155, 155],
                "the corner block has border on two sides",
            );
        }
        8 => {
            frame.assert_rgb(136, 150, [64, 191, 191], "8 px block: 6 of border, 2 of content");
            frame.assert_rgb(143, 150, [64, 191, 191], "8 px block, far end");
            frame.assert_rgb(144, 150, red, "the next block is all content");
        }
        4 => {
            frame.assert_rgb(139, 150, CYAN, "a 4 px block of border");
            frame.assert_rgb(140, 150, [128, 128, 128], "4 px block: half border, half content");
            frame.assert_rgb(143, 150, [128, 128, 128], "4 px block, far end");
            frame.assert_rgb(144, 150, red, "the next block is all content");
        }
        _ => {
            // 2 px blocks end where the border does: the picture is the sharp one.
            frame.assert_rgb(136, 150, CYAN, "border");
            frame.assert_rgb(141, 150, CYAN, "border, inner edge");
            frame.assert_rgb(142, 150, red, "content from 6 px in");
            frame.assert_rgb(140, 90, CYAN, "corner");
        }
    }
}

/// How many windows, layers and snapshots the next frame draws in blocks.
fn pixelated(f: &mut Fixture) -> usize {
    let crate::state::State { backend, hecate } = f.hecate();
    let output = hecate.wm.outputs().next().unwrap().clone();
    backend
        .with_renderer(|r| {
            hecate
                .render_elements(r, &output)
                .iter()
                .filter(|e| matches!(e, HecateElement::Window(WindowElement::Pixelated(_))))
                .count()
        })
        .unwrap()
}

#[test]
fn window_pixelates_in_halving_its_blocks() {
    let Some((mut f, id)) = with_toml(&pixelate("window_open")) else {
        return;
    };
    f.map_shm_window(id, (116, 116), RED);
    for cell in [16, 8, 4, 2] {
        let frame = f.render("headless-1");
        assert_blocks(&frame, cell);
        frame.assert_rgb(200, 150, [255, 0, 0], "in place and opaque all the way");
        assert_eq!(pixelated(&mut f), 1);
        // Stepped: the picture holds until the next step.
        f.advance_clock(99);
        assert_blocks(&f.render("headless-1"), cell);
        assert!(running(&mut f), "still animating at {cell} px");
        f.advance_clock(1);
    }
    assert_blocks(&f.render("headless-1"), 1);
    assert!(!running(&mut f), "sharp: the animation is over");
    assert_eq!(pixelated(&mut f), 0, "drawn as it is again");
}

#[test]
fn closed_window_pixelates_out_through_the_same_steps() {
    let Some((mut f, id)) = with_toml(&pixelate("window_close")) else {
        return;
    };
    let surface = f.map_shm_window(id, (116, 116), RED);
    assert_blocks(&f.render("headless-1"), 1);
    f.client(id).window(&surface).xdg_toplevel.destroy();
    f.double_roundtrip(id);
    assert!(f.windows().is_empty(), "window is gone from the WM");

    for cell in [2, 4, 8, 16] {
        assert_blocks(&f.render("headless-1"), cell);
        assert_eq!(pixelated(&mut f), 1, "the snapshot, at {cell} px");
        f.advance_clock(100);
    }
    f.render("headless-1")
        .assert_rgb(200, 150, BG, "cut once the blocks are at their coarsest");
    f.render("headless-1");
    assert!(f.hecate().hecate.wm.closing.is_empty(), "snapshot dropped");
}

#[test]
fn nothing_pixelates_with_animations_off() {
    let toml = format!("{}[animations]\nenable = false\n", pixelate("window_open"));
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    f.map_shm_window(id, (116, 116), RED);
    assert_blocks(&f.render("headless-1"), 1);
    assert!(!running(&mut f));

    // Game Mode switches them off while the session runs.
    let Some((mut f, id)) = with_toml(&pixelate("window_open")) else {
        return;
    };
    f.hecate()
        .set_override("animations.enable", json!(false))
        .unwrap();
    f.map_shm_window(id, (116, 116), RED);
    assert_blocks(&f.render("headless-1"), 1);
    assert_eq!(pixelated(&mut f), 0);
    assert!(!running(&mut f));
}

#[test]
fn layers_pixelate_in_and_out() {
    let toml = format!(
        "{}[animations.layer_close]\nstyle = \"pixelate\"\nms = 400\ncurve = \"stepped\"\nfade = false\n",
        pixelate("layer_open")
    );
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    let surface = bar(&mut f, id, "bar");
    // A bar of one colour looks the same in blocks: what tells is how it is drawn.
    f.render("headless-1")
        .assert_rgb(200, 15, [0, 0, 255], "in place and opaque from the start");
    assert_eq!(pixelated(&mut f), 1);
    assert!(running(&mut f));
    f.advance_clock(400);
    f.render("headless-1")
        .assert_rgb(200, 15, [0, 0, 255], "still there");
    assert_eq!(pixelated(&mut f), 0, "sharp");
    assert!(!running(&mut f));

    f.client(id).layer(&surface).layer_surface.destroy();
    f.double_roundtrip(id);
    f.render("headless-1")
        .assert_rgb(200, 15, [0, 0, 255], "its snapshot");
    assert_eq!(pixelated(&mut f), 1);
    f.advance_clock(400);
    f.render("headless-1").assert_rgb(200, 15, BG, "gone");
}

#[test]
fn layer_rules_choose_pixelate_or_something_else() {
    // The rule asks for pixelate where the default is a fade.
    let toml = "[general]\nbackground_color = \"#102030\"\n\
                [animations.layer_open]\nstyle = \"fade\"\nms = 400\ncurve = \"linear\"\nfade = false\n\
                [[layer_rules]]\nmatch = { namespace = \"^bar$\" }\nanimation = \"pixelate\"\n";
    let Some((mut f, id)) = with_toml(toml) else {
        return;
    };
    bar(&mut f, id, "bar");
    f.render("headless-1")
        .assert_rgb(200, 15, [0, 0, 255], "not faded");
    assert_eq!(pixelated(&mut f), 1);

    // And for a slide where the default is pixelate.
    let toml = format!(
        "{}[[layer_rules]]\nmatch = {{ namespace = \"^bar$\" }}\nanimation = \"slide\"\n",
        pixelate("layer_open")
    );
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    bar(&mut f, id, "bar");
    f.render("headless-1")
        .assert_rgb(200, 5, BG, "above the output at first");
    assert_eq!(pixelated(&mut f), 0);
}

#[test]
fn workspace_switch_is_a_cut() {
    let toml = format!("{BLOCKS}[animations.workspace_switch]\nstyle = \"none\"\n");
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    f.map_shm_window(id, (116, 116), RED);
    f.render("headless-1");
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 150, BG, "the old workspace is gone on the next frame");
    frame.assert_rgb(20, 150, BG, "and didn't slide away");
    assert!(!running(&mut f));

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(1)), None);
    assert_blocks(&f.render("headless-1"), 1);
    assert!(!running(&mut f));
}

#[test]
fn workspace_switch_can_pixelate_the_new_workspace_in() {
    let Some((mut f, id)) = with_toml(&pixelate("workspace_switch")) else {
        return;
    };
    f.map_shm_window(id, (116, 116), RED);
    f.render("headless-1");
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 150, BG, "the old workspace is cut");
    frame.assert_rgb(20, 150, BG, "nothing slides");
    f.advance_clock(400);
    f.render("headless-1");
    // A window in the middle of workspace 2.
    f.map_shm_window(id, (40, 40), BLUE);
    f.render("headless-1")
        .assert_rgb(200, 150, [0, 0, 255], "workspace 2");

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(1)), None);
    for cell in [16, 8, 4, 2] {
        let frame = f.render("headless-1");
        assert_blocks(&frame, cell);
        frame.assert_rgb(200, 150, [255, 0, 0], "workspace 2 is not drawn under it");
        assert!(running(&mut f));
        f.advance_clock(100);
    }
    assert_blocks(&f.render("headless-1"), 1);
    assert!(!running(&mut f));
}

/// The haiiro frame: a 5 px double border in pine with the label on its band.
const FRAME: &str = "[general]\nbackground_color = \"#102030\"\n\
                     [border]\nwidth = 5\nstyle = \"double\"\nradius = 0\nbackground = \"#1f1d2e\"\n\
                     [border.color]\nactive = \"#31748f\"\ninactive = \"#403d52\"\n\
                     [border.label]\nenable = true\ncolor = \"#e0def4\"\n";
const PINE: [u8; 3] = [0x31, 0x74, 0x8f];
const SURFACE: [u8; 3] = [0x1f, 0x1d, 0x2e];
const TEXT: [u8; 3] = [0xe0, 0xde, 0xf4];

fn color([r, g, b]: [u8; 3]) -> Color {
    Color::rgb(r, g, b)
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// Map a red 200×100 window that says what it is. Under `FRAME` its content is at
/// (100, 106): the frame is 5 px on three sides and 18 px on top, and it is the frame that
/// is centred.
fn titled(f: &mut Fixture, id: ClientId, app_id: &str, title: &str) -> WlSurface {
    let window = f.client(id).create_window();
    window.set_app_id(app_id);
    window.set_title(title);
    let surface = window.surface.clone();
    window.commit();
    f.double_roundtrip(id);
    let window = f.client(id).window(&surface);
    window.attach_shm(200, 100, RED);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    surface
}

fn retitle(f: &mut Fixture, id: ClientId, surface: &WlSurface, title: &str) {
    let window = f.client(id).window(surface);
    window.set_title(title);
    window.commit();
    f.double_roundtrip(id);
}

/// The label of a `titled` window reads `text`, pixel for pixel: 15 px in from the content's
/// left edge, 18 px tall, its lower half on the band.
fn assert_label(frame: &Frame, text: &str) {
    let line = label::layout(text, 1000).expect("the test's text has glyphs");
    let (pixels, w, h) = label::rasterize_line(&line, color(TEXT), color(SURFACE), 1);
    assert_eq!(h, 18);
    let mut ink = 0;
    for y in 0..h {
        for x in 0..w {
            let p = &pixels[(y * w + x) * 4..][..3];
            let expected = [p[2], p[1], p[0]];
            ink += usize::from(expected == TEXT);
            frame.assert_rgb(115 + x as i32, 88 + y as i32, expected, "label pixel");
        }
    }
    assert!(ink > 4 * text.chars().count(), "{text:?} has ink");
    frame.assert_rgb(114, 90, BG, "nothing left of the label above the line");
    frame.assert_rgb(115 + w as i32, 90, BG, "nothing right of it either");
}

#[test]
fn double_frame_with_the_label_on_its_band() {
    let Some((mut f, id)) = with_toml(FRAME) else {
        return;
    };
    titled(&mut f, id, "kitty", "~/src/wl-hex");
    let w = f.windows()[0].clone();
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(100, 106, 200, 100));
    let frame = f.render("headless-1");
    let red = [255, 0, 0];

    // Left edge, outside in: a 2 px line, a 1 px gap, a 2 px line.
    for (x, colour, what) in [
        (94, BG, "outside"),
        (95, PINE, "outer line"),
        (96, PINE, "outer line"),
        (97, SURFACE, "gap"),
        (98, PINE, "inner line"),
        (99, PINE, "inner line"),
        (100, red, "content"),
    ] {
        frame.assert_rgb(x, 150, colour, what);
    }
    // Top edge, clear of the label: the same lines, then the band the label stands on.
    for (y, colour, what) in [
        (91, BG, "above the frame"),
        (92, PINE, "outer line"),
        (93, PINE, "outer line"),
        (94, SURFACE, "gap"),
        (95, PINE, "inner line"),
        (96, PINE, "inner line"),
        (97, SURFACE, "band"),
        (105, SURFACE, "band"),
        (106, red, "content"),
    ] {
        frame.assert_rgb(290, y, colour, what);
    }
    for (y, colour, what) in [
        (205, red, "content"),
        (206, PINE, "inner line"),
        (207, PINE, "inner line"),
        (208, SURFACE, "gap"),
        (209, PINE, "outer line"),
        (210, PINE, "outer line"),
        (211, BG, "below the frame"),
    ] {
        frame.assert_rgb(200, y, colour, what);
    }
    frame.assert_rgb(95, 92, PINE, "square corner, top left");
    frame.assert_rgb(304, 210, PINE, "square corner, bottom right");
    frame.assert_rgb(97, 94, SURFACE, "the gap turns the corner");
    frame.assert_rgb(96, 90, BG, "the sides stop at the top line");

    // "kitty: ~/src/wl-hex": 19 cells of the 8x16 face and a cell of padding either side.
    assert_label(&frame, "kitty: ~/src/wl-hex");
    frame.assert_rgb(115 + 19 * 8 + 15, 97, SURFACE, "the label's last column");
}

#[test]
fn unfocused_frame_takes_the_inactive_colour() {
    let Some((mut f, id)) = with_toml(FRAME) else {
        return;
    };
    titled(&mut f, id, "kitty", "one");
    // A second, smaller window takes the focus; the first one's frame shows around it.
    f.map_shm_window(id, (40, 40), BLUE);
    let frame = f.render("headless-1");
    frame.assert_rgb(95, 150, [0x40, 0x3d, 0x52], "inactive line");
    frame.assert_rgb(97, 150, SURFACE, "the gap keeps its colour");
    assert_label(&frame, "kitty: one");
}

#[test]
fn label_follows_the_title_and_falls_back_cleanly() {
    let Some((mut f, id)) = with_toml(FRAME) else {
        return;
    };
    let surface = titled(&mut f, id, "org.gnome.Nautilus", "Home");
    assert_label(&f.render("headless-1"), "nautilus: Home");

    // Hungarian, Greek and Cyrillic are in the VGA face itself.
    retitle(&mut f, id, &surface, "őű λ Ж");
    assert_label(&f.render("headless-1"), "nautilus: őű λ Ж");

    // CJK comes from the fallback face, two cells a glyph.
    retitle(&mut f, id, &surface, "日本語");
    let frame = f.render("headless-1");
    assert_label(&frame, "nautilus: 日本語");
    frame.assert_rgb(115 + (10 + 3 * 2) * 8 + 15, 90, SURFACE, "16 cells wide");

    // A script neither face can set properly leaves the app name alone.
    retitle(&mut f, id, &surface, "שלום עולם");
    assert_label(&f.render("headless-1"), "nautilus");

    // What doesn't fit between the corners is cut: 170 px of room hold 19 cells.
    retitle(&mut f, id, &surface, "a very long title that cannot fit");
    assert_label(&f.render("headless-1"), "nautilus: a very l…");

    // No title: the app name.
    retitle(&mut f, id, &surface, "");
    assert_label(&f.render("headless-1"), "nautilus");
}

#[test]
fn window_pixelates_in_with_its_frame_and_label() {
    let toml = format!(
        "{FRAME}[animations.window_open]\nstyle = \"pixelate\"\nms = 400\ncurve = \"stepped\"\ncell = 16\nfade = false\n"
    );
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    titled(&mut f, id, "kitty", "~");
    // In blocks, the label's text is mixed into its background: no pixel has the text colour.
    let frame = f.render("headless-1");
    assert_eq!(pixelated(&mut f), 1);
    let ink = |frame: &Frame| {
        (88..106)
            .flat_map(|y| (115..115 + 8 * 8 + 16).map(move |x| (x, y)))
            .filter(|(x, y)| frame.pixel(*x, *y)[..3] == TEXT)
            .count()
    };
    assert_eq!(ink(&frame), 0, "the label is in blocks too");
    frame.assert_rgb(200, 150, [255, 0, 0], "content");
    f.advance_clock(400);
    let frame = f.render("headless-1");
    assert_label(&frame, "kitty: ~");
    assert!(ink(&frame) > 0);
}

#[test]
fn layout_makes_room_for_the_frame() {
    let Some((mut f, id)) = with_toml(FRAME) else {
        return;
    };
    let surface = f.map_window(id, (200, 100), RED, Some("kitty"));
    let w = f.windows()[0].clone();
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(100, 106, 200, 100));

    // The band and the label's row are frame: they take the pointer for the window.
    let under = |f: &mut Fixture, x: f64, y: f64| {
        let c = f.hecate().contents_under((x, y).into());
        (c.window.is_some(), c.on_border)
    };
    assert_eq!(under(&mut f, 200., 150.), (true, false), "the window itself");
    assert_eq!(under(&mut f, 200., 100.), (true, true), "the band");
    assert_eq!(under(&mut f, 200., 89.), (true, true), "the label's upper half");
    assert_eq!(under(&mut f, 97., 150.), (true, true), "the border");
    assert_eq!(under(&mut f, 200., 87.), (false, false), "above the frame");
    assert_eq!(under(&mut f, 93., 150.), (false, false), "beside it");

    // Snapped, the frame fills the half and the window what the frame leaves of it.
    f.hecate().do_action(Action::SnapToEdge(Edge::Left), None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(5, 18, 190, 277));
    f.hecate().do_action(Action::SnapToEdge(Edge::Right), None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(205, 18, 190, 277));

    // Maximized, the frame goes and the window has the whole output.
    f.hecate().do_action(Action::Maximize, None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(0, 0, 400, 300));
    assert_eq!(under(&mut f, 200., 2.), (true, false), "no frame to hit");
}

#[test]
fn a_frame_kept_on_maximize_stays_on_the_output() {
    let toml = FRAME.replace("radius = 0", "radius = 0\nkeep_on_maximize = true");
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    let surface = f.map_window(id, (200, 100), RED, Some("kitty"));
    let w = f.windows()[0].clone();
    f.hecate().do_action(Action::Maximize, None);
    f.double_roundtrip(id);
    f.ack_and_resize(id, &surface, RED);
    assert_eq!(f.hecate().hecate.wm.window_rect(&w), rect(5, 18, 390, 277));
    let frame = f.render("headless-1");
    frame.assert_rgb(0, 150, PINE, "the border's outer line on the output's edge");
    frame.assert_rgb(399, 150, PINE, "and on the other");
    frame.assert_rgb(200, 299, PINE, "bottom line");
    frame.assert_rgb(
        390,
        4,
        PINE,
        "top line, 4 px down: the label's upper half is above it",
    );
    frame.assert_rgb(390, 12, SURFACE, "band");
    frame.assert_rgb(200, 18, [255, 0, 0], "content");
}

/// A 10 px border, white when focused and black when not, around a 100×100 window at
/// (150, 100): (145, 150) is border.
const TINT: &str = "[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 10\nradius = 0\n\
                    [border.color]\nactive = \"#ffffff\"\ninactive = \"#000000\"\n";
const GOLD: [u8; 3] = [0xf6, 0xc1, 0x77];
const LOVE: [u8; 3] = [0xeb, 0x6f, 0x92];

fn tint(f: &mut Fixture, window: u64, value: Value) -> Result<(), String> {
    let reply = f.hecate().ipc_request(Request::Action {
        action: json!({ "border_tint": value }),
        target: Some(Selector::Id(window)),
    });
    match reply.expect("no reply") {
        Reply::Ok(_) => Ok(()),
        Reply::Error(e) => Err(e),
    }
}

#[test]
fn border_tint_wins_over_focus_until_it_is_cleared() {
    let Some((mut f, id)) = with_toml(TINT) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let first = f.windows()[0].wid();
    f.render("headless-1")
        .assert_rgb(145, 150, [255, 255, 255], "focused");

    // The colour snaps: `animations.border` is off here, as in the haiiro defaults.
    tint(&mut f, first, json!("gold")).unwrap();
    f.render("headless-1")
        .assert_rgb(145, 150, GOLD, "gold on the next frame");
    assert!(!running(&mut f), "nothing to animate");
    tint(&mut f, first, json!("love")).unwrap();
    f.render("headless-1").assert_rgb(145, 150, LOVE, "love");
    tint(&mut f, first, json!("#00ff00")).unwrap();
    f.render("headless-1")
        .assert_rgb(145, 150, [0, 255, 0], "any colour");

    // Unknown names are refused and change nothing.
    let err = tint(&mut f, first, json!("mauve")).unwrap_err();
    assert!(err.contains("unknown tint"), "{err}");
    f.render("headless-1")
        .assert_rgb(145, 150, [0, 255, 0], "unchanged");

    // The tint is the window's, focused or not: another window takes the focus.
    f.map_shm_window(id, (40, 40), BLUE);
    tint(&mut f, first, json!("gold")).unwrap();
    f.render("headless-1")
        .assert_rgb(145, 150, GOLD, "tinted without the focus");
    tint(&mut f, first, json!("none")).unwrap();
    f.render("headless-1")
        .assert_rgb(145, 150, [0, 0, 0], "back to its own colour: inactive");
}

#[test]
fn timed_border_tint_clears_itself() {
    let Some((mut f, id)) = with_toml(TINT) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    let window = f.windows()[0].wid();
    tint(&mut f, window, json!({ "color": "love", "ms": 30 })).unwrap();
    f.render("headless-1").assert_rgb(145, 150, LOVE, "tinted");
    std::thread::sleep(Duration::from_millis(60));
    f.dispatch();
    f.render("headless-1")
        .assert_rgb(145, 150, [255, 255, 255], "cleared after its time");

    // A tint set since is not the timer's to clear.
    tint(&mut f, window, json!({ "color": "love", "ms": 30 })).unwrap();
    tint(&mut f, window, json!("gold")).unwrap();
    std::thread::sleep(Duration::from_millis(60));
    f.dispatch();
    f.render("headless-1")
        .assert_rgb(145, 150, GOLD, "the later tint stays");
}

#[test]
fn border_tint_without_a_target_takes_the_focused_window() {
    let Some((mut f, id)) = with_toml(TINT) else {
        return;
    };
    let reply = f.hecate().ipc_request(Request::Action {
        action: json!({ "border_tint": "gold" }),
        target: None,
    });
    assert!(matches!(reply, Some(Reply::Error(_))), "no window to tint");

    f.map_shm_window(id, (100, 100), RED);
    let reply = f.hecate().ipc_request(Request::Action {
        action: json!({ "border_tint": "gold" }),
        target: None,
    });
    assert!(matches!(reply, Some(Reply::Ok(_))));
    f.render("headless-1")
        .assert_rgb(145, 150, GOLD, "focused window");
}

/// The picture of a 400×300 desktop: a red strip over y = 0..50, green below.
fn saved_desktop(name: &str) -> Option<PathBuf> {
    let dir = splash::temp_dir(name);
    let mut f = splash::fixture("#00ff00", &dir)?;
    let id = f.add_client();
    f.double_roundtrip(id);
    splash::layer(&mut f, id, Layer::Background, Anchor::Top, [255, 0, 0, 255]);
    splash::save(&mut f);
    Some(dir)
}

/// The next session, with that picture over its own black background and a clock to step.
fn next_session(dir: &Path, handover: &str, more: &str) -> Option<Fixture> {
    let mut f = Fixture::with_toml(&format!(
        "[splash]\nenable = true\nhandover = \"{handover}\"\n[border]\nwidth = 0\n\
         [general]\nbackground_color = \"#000000\"\n{more}"
    ));
    if !f.enable_rendering() {
        return None;
    }
    f.hecate().hecate.snapshot_dir = Some(dir.to_path_buf());
    f.add_output("headless-1", (400, 300));
    f.manual_clock();
    Some(f)
}

#[test]
fn splash_hands_over_with_a_cut() {
    let Some(dir) = saved_desktop("haiiro-cut") else {
        return;
    };
    let Some(mut f) = next_session(&dir, "cut", "") else {
        return;
    };
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 20, [255, 0, 0], "the picture");
    frame.assert_rgb(200, 150, [0, 255, 0], "the picture");

    f.hecate().do_action(Action::SplashDone, None);
    assert!(f.hecate().hecate.splash.is_empty(), "nothing to wait for");
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 20, [0, 0, 0], "the desktop on the next frame");
    frame.assert_rgb(200, 150, [0, 0, 0], "the desktop on the next frame");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn splash_hands_over_in_growing_blocks() {
    let Some(dir) = saved_desktop("haiiro-pixelate") else {
        return;
    };
    let Some(mut f) = next_session(&dir, "pixelate", "") else {
        return;
    };
    f.render("headless-1");
    f.hecate().do_action(Action::SplashDone, None);

    // Blocks are counted from the middle of the picture, row 150, so 2 and 4 px blocks end
    // where the strip does at row 50: nothing to see yet.
    for _ in 0..2 {
        let frame = f.render("headless-1");
        frame.assert_rgb(200, 49, [255, 0, 0], "strip");
        frame.assert_rgb(200, 50, [0, 255, 0], "below it");
        f.advance_clock(125);
    }
    // 8 px: rows 46..54 are half strip.
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 45, [255, 0, 0], "strip");
    frame.assert_rgb(200, 46, [128, 128, 0], "8 px block");
    frame.assert_rgb(200, 53, [128, 128, 0], "8 px block");
    frame.assert_rgb(200, 54, [0, 255, 0], "below it");
    f.advance_clock(125);
    // 16 px: rows 38..54 are three quarters strip.
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 37, [255, 0, 0], "strip");
    frame.assert_rgb(200, 38, [191, 64, 0], "16 px block");
    frame.assert_rgb(200, 53, [191, 64, 0], "16 px block");
    frame.assert_rgb(200, 54, [0, 255, 0], "below it");
    frame.assert_rgb(200, 150, [0, 255, 0], "opaque to the end");
    f.advance_clock(125);
    f.render("headless-1");
    f.render("headless-1")
        .assert_rgb(200, 150, [0, 0, 0], "then the desktop");
    assert!(f.hecate().hecate.splash.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn splash_is_cut_when_animations_are_off() {
    let Some(dir) = saved_desktop("haiiro-off") else {
        return;
    };
    for handover in ["pixelate", "fade"] {
        let Some(mut f) = next_session(&dir, handover, "[animations]\nenable = false\n") else {
            return;
        };
        f.render("headless-1")
            .assert_rgb(200, 150, [0, 255, 0], "the picture");
        f.hecate().do_action(Action::SplashDone, None);
        assert!(f.hecate().hecate.splash.is_empty(), "{handover}: no motion");
        f.render("headless-1")
            .assert_rgb(200, 150, [0, 0, 0], "the desktop on the next frame");
    }
    let _ = std::fs::remove_dir_all(dir);
}
