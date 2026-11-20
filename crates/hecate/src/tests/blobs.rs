//! The blob background, read back from software-rendered frames and held to the reference
//! raster (`Sim::tones`): the same blobs, summed and dithered on the CPU.

use std::time::{Duration, Instant};

use hecate_ipc::{self as ipc, Payload, Reply, Request};

use super::fixture::{Fixture, Frame};
use crate::config::action::Action;
use crate::render::blobs::Sim;

const OUTPUT: &str = "headless-1";
/// 107 by 60 cells of 6 pixels: the last column is cut after 4 pixels.
const SIZE: (i32, i32) = (640, 360);
const GROUND: [u8; 3] = [0x19, 0x17, 0x24];
const HALO: [u8; 3] = [0x26, 0x23, 0x3a];
const CORE: [u8; 3] = [0x40, 0x3d, 0x52];
const SOLID: [u8; 3] = [0x10, 0x20, 0x30];
const RED: [u8; 4] = [255, 0, 0, 255];

fn fixture(extra: &str) -> Option<Fixture> {
    let toml = format!(
        "[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 0\n[background.blobs]\nenable = true\n{extra}"
    );
    let mut f = Fixture::with_toml(&toml);
    if !f.enable_rendering() {
        return None;
    }
    // Only the tests move the blobs on.
    f.manual_clock();
    f.add_output(OUTPUT, SIZE);
    Some(f)
}

fn sim(f: &mut Fixture) -> Sim {
    f.hecate()
        .hecate
        .blobs_sim(OUTPUT)
        .expect("no blobs on the output")
}

/// One tick of the blobs' timer, `ms` later.
fn tick(f: &mut Fixture, ms: u64) -> bool {
    f.advance_clock(ms);
    f.hecate().hecate.advance_blobs()
}

/// `steps` ticks of 33 ms, each one drawn.
fn play(f: &mut Fixture, steps: u32) {
    for _ in 0..steps {
        tick(f, 33);
        f.render(OUTPUT);
    }
}

fn status(f: &mut Fixture) -> ipc::BlobsStatus {
    match f.hecate().ipc_request(Request::Blobs(ipc::Blobs::Status)) {
        Some(Reply::Ok(Payload::Value(v))) => serde_json::from_value(v).unwrap(),
        other => panic!("unexpected reply {other:?}"),
    }
}

fn paused(f: &mut Fixture) -> Option<String> {
    status(f).outputs[0].paused.clone()
}

/// How many pixels of `frame` are not the tone the reference gives their cell (of `cell`
/// pixels), and how many cells have each tone.
fn compare(frame: &Frame, sim: &Sim, cell: i32) -> (usize, [usize; 3]) {
    let tones = sim.tones();
    let (w, _) = sim.size();
    let mut wrong = 0;
    let mut seen = [0; 3];
    for tone in &tones {
        seen[*tone as usize] += 1;
    }
    for y in 0..frame.h {
        for x in 0..frame.w {
            let tone = tones[((y / cell) * w + x / cell) as usize];
            let expected = [GROUND, HALO, CORE][tone as usize];
            if frame.pixel(x, y)[..3] != expected {
                wrong += 1;
            }
        }
    }
    (wrong, seen)
}

/// The frame is the reference's picture, but for a few cells where the two round a sum
/// that sits on a threshold differently.
fn assert_matches(frame: &Frame, sim: &Sim, cell: i32, what: &str) {
    let (wrong, seen) = compare(frame, sim, cell);
    assert!(
        seen.iter().all(|n| *n > 50),
        "{what}: a picture with every tone in it, not {seen:?}"
    );
    let allowed = (4 * cell * cell) as usize;
    assert!(
        wrong <= allowed,
        "{what}: {wrong} pixels differ from the reference (four cells may)"
    );
}

#[test]
fn the_shader_draws_the_reference_picture() {
    let Some(mut f) = fixture("") else { return };
    let frame = f.render(OUTPUT);
    let sim = sim(&mut f);
    assert_eq!(sim.size(), (107, 60));
    assert_eq!(sim.blobs().len(), 9, "107 * 60 / 700");
    assert_matches(&frame, &sim, 6, "the first frame");
    // And after a while, with merged blobs and ones that drifted in.
    play(&mut f, 600);
    let frame = f.render(OUTPUT);
    assert_matches(&frame, &sim_after(&mut f, &sim), 6, "twenty seconds on");
}

/// The simulation now, which is no longer `before`.
fn sim_after(f: &mut Fixture, before: &Sim) -> Sim {
    let now = sim(f);
    assert_ne!(&now, before);
    now
}

#[test]
fn cells_are_logical_pixels() {
    let Some(mut f) = fixture("[outputs.headless-1]\nscale = 2.0") else {
        return;
    };
    let frame = f.render(OUTPUT);
    let sim = sim(&mut f);
    assert_eq!(sim.size(), (54, 30), "cells of 12 physical pixels");
    assert_eq!(status(&mut f).outputs[0].cell, 12);
    assert_matches(&frame, &sim, 12, "at scale 2");
}

#[test]
fn the_blobs_follow_the_clock() {
    // One blob: nothing to merge with.
    let Some(mut f) = fixture("count_min = 1\ncount_max = 1") else {
        return;
    };
    f.render(OUTPUT);
    let start = sim(&mut f);
    // The first tick starts the clock, the next ones move by the time that passed.
    assert!(tick(&mut f, 33));
    assert_eq!(sim(&mut f), start);
    assert!(tick(&mut f, 33));
    let moved = sim(&mut f);
    assert!((moved.time() - 0.033).abs() < 1e-6);
    let (a, b) = (start.blobs()[0], moved.blobs()[0]);
    assert!((b.x - (a.x + a.vx * 0.033)).abs() < 1e-4);
    // No time, no step: the picture stays as it is.
    assert!(tick(&mut f, 0));
    assert_eq!(sim(&mut f), moved);
    // After a stall they carry on, they don't jump.
    assert!(tick(&mut f, 5000));
    assert!((sim(&mut f).time() - 0.133).abs() < 1e-6);
}

#[test]
fn a_step_is_a_new_frame() {
    let Some(mut f) = fixture("") else { return };
    let first = f.render(OUTPUT);
    let again = f.render(OUTPUT);
    assert_eq!(first.bytes, again.bytes, "nothing moved");
    tick(&mut f, 33);
    play(&mut f, 30);
    let later = f.render(OUTPUT);
    assert_ne!(first.bytes, later.bytes);
    assert_matches(&later, &sim(&mut f), 6, "a second on");
}

#[test]
fn game_mode_pauses_the_blobs() {
    let Some(mut f) = fixture("") else { return };
    f.render(OUTPUT);
    tick(&mut f, 33);
    assert_eq!(paused(&mut f), None);
    // What the shell's Game Mode sets.
    f.hecate()
        .set_override("animations.enable", serde_json::json!(false))
        .unwrap();
    let frozen = sim(&mut f);
    assert!(!tick(&mut f, 33), "nothing moves: the timer stops");
    assert!(!tick(&mut f, 33));
    assert_eq!(sim(&mut f), frozen);
    assert_eq!(paused(&mut f).as_deref(), Some("animations_off"));
    // Still drawn, as it was.
    let frame = f.render(OUTPUT);
    assert_matches(&frame, &frozen, 6, "paused");

    f.hecate()
        .unset_override(ipc::Unset::Path("animations.enable".into()))
        .unwrap();
    assert!(tick(&mut f, 33));
    assert!(tick(&mut f, 33));
    assert_ne!(sim(&mut f), frozen);
}

#[test]
fn a_fullscreen_window_pauses_the_blobs_under_it() {
    let Some(mut f) = fixture("") else { return };
    let id = f.add_client();
    let surface = f.map_window(id, (200, 100), RED, None);
    f.render(OUTPUT);
    tick(&mut f, 33);
    assert!(tick(&mut f, 33), "a window that isn't fullscreen changes nothing");

    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &surface, RED);
    let frozen = sim(&mut f);
    assert!(!tick(&mut f, 33));
    assert_eq!(sim(&mut f), frozen);
    assert_eq!(paused(&mut f).as_deref(), Some("fullscreen"));

    f.hecate().do_action(Action::ToggleFullscreen, None);
    f.ack_and_resize(id, &surface, RED);
    assert!(tick(&mut f, 33));
    assert!(tick(&mut f, 33));
    assert_ne!(sim(&mut f), frozen);
}

#[test]
fn reduced_motion_is_one_settled_frame() {
    let Some(mut f) = fixture("still = true") else {
        return;
    };
    let frame = f.render(OUTPUT);
    let still = sim(&mut f);
    assert!((still.time() - 30.).abs() < 1e-3, "300 steps of 0.1 s");
    assert_matches(&frame, &still, 6, "the still frame");
    assert!(!tick(&mut f, 33));
    assert!(!tick(&mut f, 33));
    assert_eq!(sim(&mut f), still);
    assert_eq!(paused(&mut f).as_deref(), Some("still"));
    assert_eq!(f.render(OUTPUT).bytes, frame.bytes);

    // Turned on while they move, it stops them where they are.
    let Some(mut f) = fixture("") else { return };
    f.render(OUTPUT);
    tick(&mut f, 33);
    tick(&mut f, 33);
    let moving = sim(&mut f);
    f.hecate()
        .set_override("background.blobs.still", serde_json::json!(true))
        .unwrap();
    assert!(!tick(&mut f, 33));
    assert_eq!(sim(&mut f), moving);
}

#[test]
fn the_ipc_switch_gives_way_to_a_wallpaper_and_back() {
    let Some(mut f) = fixture("") else { return };
    f.render(OUTPUT);
    tick(&mut f, 33);
    tick(&mut f, 33);
    let before = sim(&mut f);
    assert!(status(&mut f).enabled);

    assert_eq!(
        f.hecate().ipc_request(Request::Blobs(ipc::Blobs::Enable(false))),
        Some(Reply::unit())
    );
    let frame = f.render(OUTPUT);
    for (x, y) in [(5, 5), (320, 180), (635, 355)] {
        frame.assert_rgb(x, y, SOLID, "the solid background");
    }
    assert!(!status(&mut f).enabled);
    assert_eq!(paused(&mut f).as_deref(), Some("off"));
    assert!(!tick(&mut f, 33));
    // An override: the config file is not touched and a reload keeps it.
    assert_eq!(
        f.hecate().get_config_value("background.blobs.enable"),
        Some(serde_json::json!(false))
    );

    f.hecate().ipc_request(Request::Blobs(ipc::Blobs::Enable(true)));
    let frame = f.render(OUTPUT);
    assert_eq!(sim(&mut f), before, "the same blobs, where they were");
    assert_matches(&frame, &before, 6, "back on");
}

#[test]
fn blobs_are_off_unless_asked_for() {
    let mut f = Fixture::with_toml("[general]\nbackground_color = \"#102030\"");
    if !f.enable_rendering() {
        return;
    }
    f.add_output(OUTPUT, SIZE);
    let frame = f.render(OUTPUT);
    frame.assert_rgb(320, 180, SOLID, "the solid background");
    assert!(f.hecate().hecate.blobs_sim(OUTPUT).is_none());
    assert!(!f.hecate().hecate.advance_blobs());
}

fn export(f: &mut Fixture) -> ipc::BlobsState {
    match f.hecate().ipc_request(Request::Blobs(ipc::Blobs::Export)) {
        Some(Reply::Ok(Payload::Value(v))) => serde_json::from_value(v).unwrap(),
        other => panic!("unexpected reply {other:?}"),
    }
}

#[test]
fn another_compositor_carries_on_from_exported_blobs() {
    let Some(mut greeter) = fixture("") else { return };
    greeter.render(OUTPUT);
    play(&mut greeter, 90);
    let last = greeter.render(OUTPUT);
    let state = export(&mut greeter);
    assert_eq!(state.outputs.len(), 1);
    assert_eq!(state.outputs[0].output, OUTPUT);
    assert_eq!((state.outputs[0].width, state.outputs[0].height), (107, 60));

    // Handed over before the session's first frame: that frame is the greeter's last one.
    let Some(mut session) = fixture("") else { return };
    session
        .hecate()
        .ipc_request(Request::Blobs(ipc::Blobs::Import(state.clone())));
    let first = session.render(OUTPUT);
    assert_eq!(first.bytes, last.bytes);
    assert_eq!(sim(&mut session), sim(&mut greeter));
    // And from there on both draw the same.
    tick(&mut session, 33);
    for _ in 0..300 {
        play(&mut greeter, 1);
        play(&mut session, 1);
    }
    assert_eq!(sim(&mut session), sim(&mut greeter));

    // Over blobs that are already there, too.
    let Some(mut other) = fixture("") else { return };
    other.render(OUTPUT);
    assert_ne!(sim(&mut other), sim(&mut greeter));
    other
        .hecate()
        .ipc_request(Request::Blobs(ipc::Blobs::Import(export(&mut greeter))));
    assert_eq!(sim(&mut other), sim(&mut greeter));
    assert_eq!(other.render(OUTPUT).bytes, greeter.render(OUTPUT).bytes);
}

#[test]
fn blobs_are_saved_at_exit_and_resumed_at_start() {
    let dir = std::env::temp_dir().join(format!("hecate-blobs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("blobs.json");
    let _ = std::fs::remove_file(&path);

    let Some(mut greeter) = fixture(&format!("save = {:?}", path.to_str().unwrap())) else {
        return;
    };
    greeter.render(OUTPUT);
    play(&mut greeter, 30);
    let last = greeter.render(OUTPUT);
    greeter.hecate().hecate.save_blobs();
    assert!(path.exists());

    let Some(mut session) = fixture(&format!("resume = {:?}", path.to_str().unwrap())) else {
        return;
    };
    let first = session.render(OUTPUT);
    assert_eq!(first.bytes, last.bytes);
    let (resumed, saved) = (sim(&mut session), sim(&mut greeter));
    assert_eq!(resumed.blobs(), saved.blobs());
    // (JSON gives a time back to within a rounding.)
    assert!((resumed.time() - saved.time()).abs() < 1e-9);
    // What drifts in next is the same as well.
    tick(&mut session, 33);
    for _ in 0..600 {
        play(&mut greeter, 1);
        play(&mut session, 1);
    }
    assert_eq!(sim(&mut session).blobs(), sim(&mut greeter).blobs());

    // A file that isn't there, or isn't blobs, is no reason not to start.
    std::fs::write(&path, "not json").unwrap();
    let Some(mut session) = fixture(&format!("resume = {:?}", path.to_str().unwrap())) else {
        return;
    };
    let frame = session.render(OUTPUT);
    assert_matches(&frame, &sim(&mut session), 6, "a fresh start");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_blobs_show_behind_a_lock_screen() {
    let Some(mut f) = fixture("") else { return };
    let id = f.add_client();
    f.double_roundtrip(id);
    f.render(OUTPUT);
    tick(&mut f, 33);
    f.client(id).lock_session();
    f.double_roundtrip(id);
    f.double_roundtrip(id);
    assert!(f.client(id).state.session_locked);
    // The lock surface has no buffer: what is behind it shows, and keeps moving.
    let frame = f.render(OUTPUT);
    assert_matches(&frame, &sim(&mut f), 6, "locked");
    let before = sim(&mut f);
    assert!(tick(&mut f, 33));
    assert_ne!(sim(&mut f), before);
}

#[test]
fn a_new_grid_keeps_the_blobs_in_place() {
    let Some(mut f) = fixture("") else { return };
    f.render(OUTPUT);
    let before = sim(&mut f);
    f.hecate()
        .set_override("background.blobs.cell", serde_json::json!(12))
        .unwrap();
    let frame = f.render(OUTPUT);
    let after = sim(&mut f);
    assert_eq!(after.size(), (54, 30));
    assert_eq!(after.blobs().len(), before.blobs().len());
    let (a, b) = (before.blobs()[0], after.blobs()[0]);
    assert!((b.x - a.x * 54. / 107.).abs() < 1e-4 && (b.y - a.y * 0.5).abs() < 1e-4);
    assert_matches(&frame, &after, 12, "on the new grid");
}

#[test]
fn blobs_nobody_draws_come_to_rest() {
    let Some(mut f) = fixture("") else { return };
    f.render(OUTPUT);
    tick(&mut f, 33);
    // The session is on another VT: steps, and no frame to show them.
    for _ in 0..9 {
        assert!(tick(&mut f, 33));
    }
    let resting = sim(&mut f);
    assert!(!tick(&mut f, 33), "the timer stops");
    assert!(!tick(&mut f, 33));
    assert_eq!(sim(&mut f), resting);
    // The next frame wakes them, and they carry on from where they were.
    let frame = f.render(OUTPUT);
    assert_matches(&frame, &resting, 6, "back");
    assert!(tick(&mut f, 5000));
    assert_eq!(sim(&mut f), resting, "the time away is not made up for");
    assert!(tick(&mut f, 33));
    assert!((sim(&mut f).time() - resting.time() - 0.033).abs() < 1e-6);
}

/// Dispatch the event loop for `ms` of real time.
fn run_for(f: &mut Fixture, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        f.dispatch();
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_timer_steps_the_blobs_while_they_move() {
    let Some(mut f) = fixture("") else { return };
    // The real clock: the timer is all that moves them.
    f.hecate().hecate.wm.clock = Default::default();
    assert!(!f.hecate().hecate.blobs_ticking());
    f.render(OUTPUT);
    assert!(f.hecate().hecate.blobs_ticking(), "started by the first frame");

    // 30 times a second at most, however often frames are drawn.
    run_for(&mut f, 100);
    let from = sim(&mut f).time();
    let (mut steps, mut last) = (0, from);
    let until = Instant::now() + Duration::from_millis(500);
    while Instant::now() < until {
        f.dispatch();
        f.render(OUTPUT);
        let time = sim(&mut f).time();
        if time != last {
            (steps, last) = (steps + 1, time);
        }
    }
    assert!((4..=17).contains(&steps), "{steps} steps in half a second");
    assert!(
        last - from < 0.7,
        "the blobs keep time, not {} s in 0.5 s",
        last - from
    );

    // Nothing moves: the timer goes, until a frame shows them moving again.
    f.hecate()
        .set_override("background.blobs.still", serde_json::json!(true))
        .unwrap();
    run_for(&mut f, 150);
    assert!(!f.hecate().hecate.blobs_ticking());
    let still = sim(&mut f);
    run_for(&mut f, 100);
    assert_eq!(sim(&mut f), still);
    f.hecate()
        .unset_override(ipc::Unset::Path("background.blobs.still".into()))
        .unwrap();
    f.render(OUTPUT);
    assert!(f.hecate().hecate.blobs_ticking());
    run_for(&mut f, 200);
    assert_ne!(sim(&mut f), still);
}
