//! The login splash: a picture of the desktop taken at lock or exit, shown from the next
//! session's first frame until the shell is ready.
//!
//! The desktop on a 400×300 output: a red wallpaper strip over y = 0..50, a blue panel over
//! y = 250..300, and a white window in the middle that the picture must leave out.

use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;

use super::client::{ClientId, LayerConfigureProps};
use super::fixture::Fixture;

const CONFIG: &str = "[splash]\nenable = true\n[border]\nwidth = 0\n[animations]\nenable = false\n";

fn fixture(background: &str, dir: &std::path::Path) -> Option<Fixture> {
    let mut f = Fixture::with_toml(&format!(
        "{CONFIG}[general]\nbackground_color = \"{background}\"\n"
    ));
    if !f.enable_rendering() {
        return None;
    }
    f.hecate().hecate.snapshot_dir = Some(dir.to_path_buf());
    f.add_output("headless-1", (400, 300));
    Some(f)
}

fn layer(f: &mut Fixture, id: ClientId, layer: Layer, anchor: Anchor, rgba: [u8; 4]) {
    let l = f.client(id).create_layer(None, layer, "shell");
    let surface = l.surface.clone();
    l.set_configure_props(LayerConfigureProps {
        size: Some((0, 50)),
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
}

fn save(f: &mut Fixture) {
    let crate::state::State { backend, hecate } = f.hecate();
    for handle in backend.with_renderer(|r| hecate.save_snapshots(r)).unwrap() {
        handle.join().unwrap();
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hecate-splash-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn snapshot_has_the_shell_but_not_the_windows_and_shows_until_the_shell_is_ready() {
    let dir = temp_dir("roundtrip");
    let Some(mut f) = fixture("#00ff00", &dir) else {
        return;
    };
    let id = f.add_client();
    f.double_roundtrip(id);
    layer(&mut f, id, Layer::Background, Anchor::Top, [255, 0, 0, 255]);
    layer(&mut f, id, Layer::Top, Anchor::Bottom, [0, 0, 255, 255]);
    f.map_window(id, (100, 100), [255, 255, 255, 255], None);
    save(&mut f);

    let (pixels, w, h) = crate::splash::decode(&dir.join("headless-1.png")).unwrap();
    assert_eq!((w, h), (400, 300));
    let at = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 3];
    assert_eq!(at(200, 20), [255, 0, 0], "wallpaper in the picture");
    assert_eq!(at(200, 280), [0, 0, 255], "panel in the picture");
    assert_eq!(at(200, 150), [0, 255, 0], "no window in the picture");

    // The next session, still without a shell, shows the picture over its black background.
    let Some(mut next) = fixture("#000000", &dir) else {
        return;
    };
    next.manual_clock();
    let frame = next.render("headless-1");
    frame.assert_rgb(200, 20, [255, 0, 0], "splash wallpaper");
    frame.assert_rgb(200, 150, [0, 255, 0], "splash background");

    // The shell is ready: the splash fades out.
    next.hecate()
        .do_action(crate::config::action::Action::SplashDone, None);
    next.render("headless-1");
    next.advance_clock(500);
    next.render("headless-1");
    let frame = next.render("headless-1");
    frame.assert_rgb(200, 150, [0, 0, 0], "real background after the fade");
    assert!(next.hecate().hecate.splash.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_off_shows_the_pictures_without_writing_any() {
    let dir = temp_dir("save-off");
    let Some(mut f) = fixture("#00ff00", &dir) else {
        return;
    };
    save(&mut f);
    assert!(dir.join("headless-1.png").is_file());

    // The login screen's compositor: starts on the session's pictures, leaves none of its own.
    let mut next = Fixture::with_toml(
        "[splash]\nenable = true\nsave = false\n[border]\nwidth = 0\n[animations]\nenable = false\n[general]\nbackground_color = \"#000000\"\n",
    );
    if !next.enable_rendering() {
        return;
    }
    next.hecate().hecate.snapshot_dir = Some(dir.clone());
    next.add_output("headless-1", (400, 300));
    next.render("headless-1").assert_rgb(
        200,
        150,
        [0, 255, 0],
        "the session's picture from the first frame",
    );
    let before = std::fs::metadata(dir.join("headless-1.png"))
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let crate::state::State { backend, hecate } = next.hecate();
    let handles = backend.with_renderer(|r| hecate.save_snapshots(r)).unwrap();
    assert!(handles.is_empty(), "nothing is written with save = false");
    let after = std::fs::metadata(dir.join("headless-1.png"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before, after, "the session's picture is left alone");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_picture_of_another_size_is_not_shown() {
    let dir = temp_dir("size");
    let Some(mut f) = fixture("#00ff00", &dir) else {
        return;
    };
    save(&mut f);
    let mut next = Fixture::with_toml(&format!("{CONFIG}[general]\nbackground_color = \"#000000\"\n"));
    if !next.enable_rendering() {
        return;
    }
    next.hecate().hecate.snapshot_dir = Some(dir.clone());
    next.add_output("headless-1", (640, 480));
    next.render("headless-1")
        .assert_rgb(320, 240, [0, 0, 0], "no stretched splash");
    let _ = std::fs::remove_dir_all(dir);
}
