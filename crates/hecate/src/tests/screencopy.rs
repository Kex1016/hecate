//! wlr-screencopy: what grim and screen recorders get back.
//!
//! Same scene as the render tests: 400×300 output, background #102030, a 100×100 red window
//! centred at (150, 100)..(250, 200).

use super::fixture::Fixture;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BG: [u8; 3] = [0x10, 0x20, 0x30];

fn fixture() -> Option<(
    Fixture,
    super::client::ClientId,
    wayland_client::protocol::wl_surface::WlSurface,
)> {
    let mut f = Fixture::with_toml("[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 0");
    if !f.enable_rendering() {
        return None;
    }
    f.add_output("headless-1", (400, 300));
    let id = f.add_client();
    let surface = f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1");
    Some((f, id, surface))
}

#[test]
fn capture_output_to_shm() {
    let Some((mut f, id, _)) = fixture() else { return };
    let o = f.client(id).output("headless-1");
    let frame = f.client(id).state.capture_output(&o, false);
    f.double_roundtrip(id);
    let info = f.client(id).state.copy_frame(&frame);
    let (format, w, h, stride) = info.shm.expect("buffer event");
    assert_eq!(format, wayland_client::protocol::wl_shm::Format::Xrgb8888 as u32);
    assert_eq!((w, h, stride), (400, 300, 1600));
    assert!(info.buffer_done, "v3 sends buffer_done");

    let buf = f.client(id).state.capture_buffer(&frame);
    frame.copy(&buf.buffer);
    f.double_roundtrip(id);
    let info = f.client(id).state.copy_frame(&frame);
    assert!(info.ready && !info.failed, "{info:?}");
    assert_eq!(buf.rgb(5, 5), BG, "background");
    assert_eq!(buf.rgb(200, 150), [255, 0, 0], "window");
    assert_eq!(buf.rgb(260, 150), BG, "right of the window");
}

#[test]
fn capture_region() {
    let Some((mut f, id, _)) = fixture() else { return };
    let o = f.client(id).output("headless-1");
    let frame = f.client(id).state.capture_region(&o, 140, 90, 40, 30);
    f.double_roundtrip(id);
    let (_, w, h, _) = f.client(id).state.copy_frame(&frame).shm.unwrap();
    assert_eq!((w, h), (40, 30));
    let buf = f.client(id).state.capture_buffer(&frame);
    frame.copy(&buf.buffer);
    f.double_roundtrip(id);
    assert!(f.client(id).state.copy_frame(&frame).ready);
    // Region (140, 90): the window starts at (150, 100), i.e. (10, 10) in the region.
    assert_eq!(buf.rgb(5, 5), BG);
    assert_eq!(buf.rgb(15, 15), [255, 0, 0]);
    assert_eq!(buf.rgb(39, 29), [255, 0, 0]);
}

#[test]
fn copy_with_damage_waits_for_changes() {
    let Some((mut f, id, surface)) = fixture() else {
        return;
    };
    let o = f.client(id).output("headless-1");

    // First damage-tracked copy has everything damaged.
    let frame = f.client(id).state.capture_output(&o, false);
    f.double_roundtrip(id);
    let buf = f.client(id).state.capture_buffer(&frame);
    frame.copy_with_damage(&buf.buffer);
    f.double_roundtrip(id);
    f.render("headless-1");
    f.double_roundtrip(id);
    let info = f.client(id).state.copy_frame(&frame);
    assert!(info.ready, "first frame: {info:?}");
    assert_eq!(info.damage, vec![(0, 0, 400, 300)], "full damage first");

    // Nothing changes: the next one waits.
    let frame = f.client(id).state.capture_output(&o, false);
    f.double_roundtrip(id);
    let buf = f.client(id).state.capture_buffer(&frame);
    frame.copy_with_damage(&buf.buffer);
    f.double_roundtrip(id);
    f.render("headless-1");
    f.double_roundtrip(id);
    assert!(
        !f.client(id).state.copy_frame(&frame).ready,
        "no damage, no frame"
    );

    // Repaint the window: the frame arrives with damage over the window only.
    let w = f.client(id).window(&surface);
    w.attach_shm(100, 100, BLUE);
    w.commit();
    f.double_roundtrip(id);
    f.render("headless-1");
    f.double_roundtrip(id);
    let info = f.client(id).state.copy_frame(&frame);
    assert!(info.ready, "frame after damage: {info:?}");
    assert!(!info.damage.is_empty());
    for (x, y, w, h) in &info.damage {
        assert!(
            *x >= 150 && *y >= 100 && x + w <= 250 && y + h <= 200,
            "damage {:?} outside the window",
            (x, y, w, h)
        );
    }
    assert_eq!(buf.rgb(200, 150), [0, 0, 255], "new content");
}

/// hyprland-toplevel-export: QuickShell's window previews.
#[test]
fn toplevel_export_copies_the_window_and_waits_for_damage() {
    let Some((mut f, id, surface)) = fixture() else {
        return;
    };
    f.client(id).window(&surface).set_app_id("red");
    f.client(id).window(&surface).commit();
    f.double_roundtrip(id);
    let handle = f
        .client(id)
        .state
        .wlr_toplevel("red")
        .expect("wlr toplevel")
        .0
        .clone();

    let frame = f.client(id).state.export_toplevel(&handle);
    f.double_roundtrip(id);
    let info = f.client(id).state.export_frame(&frame);
    let (format, w, h, stride) = info.shm.expect("buffer event");
    assert_eq!(format, wayland_client::protocol::wl_shm::Format::Xrgb8888 as u32);
    assert_eq!(
        (w, h, stride),
        (100, 100, 400),
        "the window's size, not the output's"
    );
    assert!(info.buffer_done);

    let buf = f.client(id).state.export_buffer(&frame);
    frame.copy(&buf.buffer, 0);
    f.double_roundtrip(id);
    let info = f.client(id).state.export_frame(&frame);
    assert!(info.ready && !info.failed, "{info:?}");
    assert_eq!(buf.rgb(50, 50), [255, 0, 0], "window contents");
    assert_eq!(buf.rgb(0, 0), [255, 0, 0], "no output background around it");

    // Nothing changed: the next frame waits until the window draws again.
    let frame = f.client(id).state.export_toplevel(&handle);
    f.double_roundtrip(id);
    let buf = f.client(id).state.export_buffer(&frame);
    frame.copy(&buf.buffer, 0);
    f.double_roundtrip(id);
    assert!(!f.client(id).state.export_frame(&frame).ready, "waits for damage");
    f.client(id).window(&surface).attach_shm(100, 100, BLUE);
    f.client(id).window(&surface).commit();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    let info = f.client(id).state.export_frame(&frame);
    assert!(info.ready, "ready once the window changed: {info:?}");
    assert_eq!(buf.rgb(50, 50), [0, 0, 255]);

    // ignore_damage copies at once.
    let frame = f.client(id).state.export_toplevel(&handle);
    f.double_roundtrip(id);
    let buf = f.client(id).state.export_buffer(&frame);
    frame.copy(&buf.buffer, 1);
    f.double_roundtrip(id);
    assert!(f.client(id).state.export_frame(&frame).ready);
}
