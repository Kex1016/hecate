//! wp-fifo and wp-commit-timing: commits held back until the compositor lets them through.

use std::time::Duration;

use super::fixture::Fixture;
use crate::config::action::{Action, WorkspaceRef};

const RED: [u8; 4] = [255, 0, 0, 255];

/// Committed window size as the compositor sees it.
fn size(f: &mut Fixture) -> (i32, i32) {
    let w = f.windows()[0].clone();
    let g = w.geometry().size;
    (g.w, g.h)
}

#[test]
fn fifo_wait_on_hidden_window_is_released_by_the_fallback() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let s = f.map_window(id, (400, 300), RED, Some("game"));
    // Hide the window: it won't be repainted.
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);

    let fifo = {
        let st = &f.client(id).state;
        st.fifo_manager.as_ref().unwrap().get_fifo(&s, &st.qh, ())
    };
    fifo.set_barrier();
    f.client(id).window(&s).commit();
    f.double_roundtrip(id);

    // Next frame waits on the barrier.
    fifo.wait_barrier();
    let w = f.client(id).window(&s);
    w.set_size(500, 300);
    w.commit();
    f.double_roundtrip(id);
    assert_eq!(size(&mut f), (400, 300), "held by the fifo barrier");

    f.hecate().sync_fallback();
    f.double_roundtrip(id);
    assert_eq!(size(&mut f), (500, 300), "released");
}

#[test]
fn fifo_wait_on_visible_window_is_released_by_a_repaint() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let s = f.map_window(id, (400, 300), RED, Some("game"));
    let fifo = {
        let st = &f.client(id).state;
        st.fifo_manager.as_ref().unwrap().get_fifo(&s, &st.qh, ())
    };
    fifo.set_barrier();
    f.client(id).window(&s).commit();
    f.double_roundtrip(id);
    fifo.wait_barrier();
    let w = f.client(id).window(&s);
    w.set_size(500, 300);
    w.commit();
    // The headless output repaints on its own (the commit queued a redraw).
    for _ in 0..5 {
        f.double_roundtrip(id);
    }
    assert_eq!(size(&mut f), (500, 300));
}

#[test]
fn timed_commit_lands_after_its_deadline() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    let s = f.map_window(id, (400, 300), RED, Some("game"));
    let timer = {
        let st = &f.client(id).state;
        st.commit_timing_manager
            .as_ref()
            .unwrap()
            .get_timer(&s, &st.qh, ())
    };
    let when = crate::utils::monotonic_time() + Duration::from_millis(150);
    let secs = when.as_secs();
    timer.set_timestamp(
        (secs >> 32) as u32,
        (secs & 0xffff_ffff) as u32,
        when.subsec_nanos(),
    );
    let w = f.client(id).window(&s);
    w.set_size(600, 300);
    w.commit();
    f.double_roundtrip(id);
    assert_eq!(size(&mut f), (400, 300), "not before the deadline");

    let start = std::time::Instant::now();
    while size(&mut f) != (600, 300) {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed commit never landed"
        );
        std::thread::sleep(Duration::from_millis(10));
        f.double_roundtrip(id);
    }
    assert!(crate::utils::monotonic_time() >= when);
}
