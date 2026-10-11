//! Animations, checked by stepping a frozen clock and reading pixels back.
//!
//! The output is 400×300 and a 100×100 window maps centred at (150, 100)..(250, 200). Every
//! animation here is linear over 100 ms, so at 50 ms it's exactly halfway.

use super::client::ClientId;
use super::fixture::Fixture;
use crate::config::action::{Action, Delta, WorkspaceRef};

const RED: [u8; 4] = [255, 0, 0, 255];
const BG: [u8; 3] = [0x10, 0x20, 0x30];
/// Red at 50% over the background.
const HALF: [u8; 3] = [135, 16, 24];

fn fixture(extra: &str) -> Option<(Fixture, ClientId)> {
    let toml = format!("[general]\nbackground_color = \"#102030\"\n[border]\nwidth = 0\nradius = 0\n{extra}");
    with_toml(&toml)
}

/// Animations the test config doesn't set are switched off, so only one plays at a time.
fn with_toml(toml: &str) -> Option<(Fixture, ClientId)> {
    let mut config = crate::config::load::from_toml_str(toml).expect("test config must parse");
    let raw: toml::Table = toml.parse().unwrap();
    let set = raw
        .get("animations")
        .and_then(|a| a.as_table())
        .cloned()
        .unwrap_or_default();
    let a = &mut config.animations;
    for (name, anim) in [
        ("window_open", &mut a.window_open),
        ("window_close", &mut a.window_close),
        ("window_move", &mut a.window_move),
        ("border", &mut a.border),
        ("layer_open", &mut a.layer_open),
        ("layer_close", &mut a.layer_close),
        ("workspace_switch", &mut a.workspace_switch),
    ] {
        if !set.contains_key(name) {
            anim.style = crate::config::AnimStyle::None;
        }
    }
    let mut f = Fixture::with_config(config);
    if !f.enable_rendering() {
        return None;
    }
    f.add_output("headless-1", (400, 300));
    f.manual_clock();
    let id = f.add_client();
    Some((f, id))
}

fn anim(name: &str, style: &str) -> String {
    format!(
        "[animations.{name}]\nstyle = \"{style}\"\nfrom = 0.5\nms = 100\ncurve = \"linear\"\nfade = false\n"
    )
}

fn running(f: &mut Fixture) -> bool {
    let o = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    f.hecate().hecate.output_state[&o].animations_running
}

#[test]
fn window_pops_in_from_its_centre() {
    let Some((mut f, id)) = fixture(&anim("window_open", "popin")) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);

    // Half size at first: 175..225.
    let frame = f.render("headless-1");
    frame.assert_rgb(200, 150, [255, 0, 0], "centre is shown from the start");
    frame.assert_rgb(170, 150, BG, "outside the half-size window");
    assert!(running(&mut f), "the output keeps redrawing while animating");

    f.advance_clock(50);
    // Three quarters: 162.5..237.5.
    let frame = f.render("headless-1");
    frame.assert_rgb(170, 150, [255, 0, 0], "grown past x = 170");
    frame.assert_rgb(158, 150, BG, "not full size yet");

    f.advance_clock(50);
    let frame = f.render("headless-1");
    frame.assert_rgb(152, 150, [255, 0, 0], "full size");
    // The frame that shows the end state also ends the animation.
    assert!(!running(&mut f), "animation finished");
}

#[test]
fn window_fades_in() {
    let Some((mut f, id)) = fixture(&anim("window_open", "fade")) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1")
        .assert_rgb(200, 150, BG, "invisible at the start");
    f.advance_clock(50);
    f.render("headless-1")
        .assert_rgb(200, 150, HALF, "half opaque halfway");
    f.advance_clock(50);
    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "opaque at the end");
}

#[test]
fn closed_window_fades_out_from_a_snapshot() {
    let Some((mut f, id)) = fixture(&anim("window_close", "fade")) else {
        return;
    };
    let surface = f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1");
    f.client(id).window(&surface).xdg_toplevel.destroy();
    f.double_roundtrip(id);
    assert!(f.windows().is_empty(), "window is gone from the WM");

    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "snapshot shown at first");
    f.advance_clock(50);
    f.render("headless-1").assert_rgb(200, 150, HALF, "half faded");
    f.advance_clock(50);
    f.render("headless-1").assert_rgb(200, 150, BG, "gone");
    f.render("headless-1");
    assert!(f.hecate().hecate.wm.closing.is_empty(), "snapshot dropped");
}

#[test]
fn programmatic_move_slides() {
    let Some((mut f, id)) = fixture(&anim("window_move", "slide")) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1");
    f.hecate()
        .do_action(Action::MoveRelative(Delta { x: 100, y: 0 }), None);

    let frame = f.render("headless-1");
    frame.assert_rgb(160, 150, [255, 0, 0], "starts where it was");
    frame.assert_rgb(300, 150, BG, "not at the target yet");
    f.advance_clock(50);
    let frame = f.render("headless-1");
    frame.assert_rgb(160, 150, BG, "left the old place");
    frame.assert_rgb(290, 150, [255, 0, 0], "halfway: 200..300");
    f.advance_clock(50);
    let frame = f.render("headless-1");
    frame.assert_rgb(300, 150, [255, 0, 0], "arrived: 250..350");
    frame.assert_rgb(240, 150, BG, "nothing left behind");
}

#[test]
fn interactive_move_is_not_animated() {
    use crate::wm::WindowExt;
    let Some((mut f, id)) = fixture(&anim("window_move", "slide")) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1");
    let w = f.windows()[0].clone();
    w.data().moving = true;
    w.data().loc.x += 100;
    f.render("headless-1")
        .assert_rgb(300, 150, [255, 0, 0], "follows the pointer at once");
}

#[test]
fn workspace_switch_slides_the_old_workspace_out() {
    let Some((mut f, id)) = fixture(&anim("workspace_switch", "slide")) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1");
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);

    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "old workspace still in place");
    f.advance_clock(50);
    // Moving to a higher workspace pushes the old one out to the left: -50..50.
    let frame = f.render("headless-1");
    frame.assert_rgb(20, 150, [255, 0, 0], "slid half an output to the left");
    frame.assert_rgb(200, 150, BG, "its old place is empty");
    f.advance_clock(50);
    let frame = f.render("headless-1");
    frame.assert_rgb(20, 150, BG, "slid out");
    assert!(!running(&mut f));

    // And back: workspace 1 comes in from the left.
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(1)), None);
    f.render("headless-1");
    f.advance_clock(50);
    let frame = f.render("headless-1");
    frame.assert_rgb(20, 150, [255, 0, 0], "coming in from the left");
    f.advance_clock(50);
    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "back in place");
}

#[test]
fn focus_change_fades_the_border_colour() {
    let extra = "[border]\nwidth = 10\nradius = 0\n[border.color]\nactive = \"#ffffff\"\ninactive = \"#000000\"\n\
                 [animations.border]\nstyle = \"fade\"\nms = 100\ncurve = \"linear\"\n";
    let toml = format!("[general]\nbackground_color = \"#102030\"\n{extra}");
    let Some((mut f, id)) = with_toml(&toml) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1")
        .assert_rgb(145, 150, [255, 255, 255], "focused: active colour");
    // Focus moves to a second window placed elsewhere; the first fades to inactive.
    f.map_shm_window(id, (50, 50), RED);
    let first = f.windows()[0].clone();
    {
        use crate::wm::WindowExt;
        first.data().loc = (20, 20).into();
        first.data().anims.last_target = None;
    }
    f.render("headless-1");
    f.advance_clock(50);
    f.render("headless-1")
        .assert_rgb(15, 70, [128, 128, 128], "halfway between active and inactive");
    f.advance_clock(50);
    f.render("headless-1")
        .assert_rgb(15, 70, [0, 0, 0], "inactive colour");
}

#[test]
fn no_animations_rule_and_global_switch() {
    let extra = format!(
        "{}[[window_rules]]\nno_animations = true\n",
        anim("window_open", "fade")
    );
    let Some((mut f, id)) = fixture(&extra) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "rule: no fade-in");

    let extra = format!("{}[animations]\nenable = false\n", anim("window_open", "fade"));
    let Some((mut f, id)) = fixture(&extra) else {
        return;
    };
    f.map_shm_window(id, (100, 100), RED);
    f.render("headless-1")
        .assert_rgb(200, 150, [255, 0, 0], "animations off: no fade-in");
}
