//! ext-image-copy-capture of outputs and toplevels (QuickShell ScreencopyView, grim).
//!
//! Scene: 400×300 output, background #102030, a 100×100 red window centred at
//! (150, 100)..(250, 200).

use super::client::CaptureBuffer;
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
    f.double_roundtrip(id);
    Some((f, id, surface))
}

fn buffer(f: &mut Fixture, id: super::client::ClientId, w: i32, h: i32) -> CaptureBuffer {
    let st = &f.client(id).state;
    CaptureBuffer::new(st.shm.as_ref().unwrap(), &st.qh, w, h)
}

#[test]
fn output_capture_and_damage() {
    let Some((mut f, id, surface)) = fixture() else {
        return;
    };
    let o = f.client(id).output("headless-1");
    let session = f.client(id).state.capture_session_output(&o);
    f.double_roundtrip(id);
    let s = f.client(id).state.capture_session_state(&session);
    assert_eq!(s.buffer_size, Some((400, 300)));
    assert!(
        s.shm_formats
            .contains(&(wayland_client::protocol::wl_shm::Format::Xrgb8888 as u32))
    );
    assert!(s.done >= 1);

    let buf = buffer(&mut f, id, 400, 300);
    let frame = f.client(id).state.capture_frame(&session, &buf);
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    let fs = f.client(id).state.capture_frame_state(&frame);
    assert!(fs.ready && !fs.failed, "{fs:?}");
    assert_eq!(buf.rgb(5, 5), BG);
    assert_eq!(buf.rgb(200, 150), [255, 0, 0]);

    // Next frame waits for damage.
    frame.destroy();
    let frame = f.client(id).state.capture_frame(&session, &buf);
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    assert!(
        !f.client(id).state.capture_frame_state(&frame).ready,
        "no damage, no frame"
    );

    let w = f.client(id).window(&surface);
    w.attach_shm(100, 100, BLUE);
    w.commit();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    let fs = f.client(id).state.capture_frame_state(&frame);
    assert!(fs.ready, "{fs:?}");
    assert_eq!(buf.rgb(200, 150), [0, 0, 255]);
}

#[test]
fn toplevel_capture_and_stop_on_close() {
    let Some((mut f, id, surface)) = fixture() else {
        return;
    };
    let bar = f.add_client();
    f.double_roundtrip(bar);
    let handle = f.client(bar).state.ext_toplevels[0].0.clone();
    let session = f.client(bar).state.capture_session_toplevel(&handle);
    f.double_roundtrip(bar);
    let s = f.client(bar).state.capture_session_state(&session);
    assert_eq!(s.buffer_size, Some((100, 100)), "window-sized buffer");

    let buf = buffer(&mut f, bar, 100, 100);
    let frame = f.client(bar).state.capture_frame(&session, &buf);
    f.double_roundtrip(bar);
    f.double_roundtrip(bar);
    assert!(f.client(bar).state.capture_frame_state(&frame).ready);
    assert_eq!(buf.rgb(0, 0), [255, 0, 0], "window content only");
    assert_eq!(buf.rgb(99, 99), [255, 0, 0]);

    let w = f.client(id).window(&surface);
    w.attach_null();
    w.commit();
    f.double_roundtrip(id);
    f.double_roundtrip(bar);
    assert!(
        f.client(bar).state.capture_session_state(&session).stopped,
        "stopped when the window goes"
    );
}
