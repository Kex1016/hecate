//! Touch, touchpad gestures and per-device input settings.

use smithay::backend::input::{InputTime, TouchSlot};

use super::client::{ClientId, TouchEvent};
use super::fixture::Fixture;
use crate::config::DeviceKind;
use crate::wm::WindowExt;

const RED: [u8; 4] = [255, 0, 0, 255];

fn time() -> InputTime {
    InputTime::from_millis(1)
}

fn slot(i: u32) -> TouchSlot {
    Some(i).into()
}

fn setup(toml: &str) -> (Fixture, ClientId) {
    let mut f = Fixture::with_toml(toml);
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    (f, id)
}

fn touch_events(f: &mut Fixture, id: ClientId) -> Vec<TouchEvent> {
    let seat = f.client(id).state.seats.values_mut().next().unwrap();
    std::mem::take(&mut seat.touch_events)
}

#[test]
fn device_settings_precedence() {
    let config = crate::config::load::from_toml_str(
        r#"
        [input."*"]
        natural_scroll = false
        pointer_speed = 0.1
        [input."type:touchpad"]
        natural_scroll = true
        [input."type:touch"]
        map_to_output = "DP-1"
        [input."Wacom Pen"]
        map_to_output = "HDMI-A-1"
        "#,
    )
    .unwrap();
    let touchpad = DeviceKind {
        touchpad: true,
        pointer: true,
        ..Default::default()
    };
    let c = config.input_for("Some Touchpad", touchpad);
    assert_eq!(c.natural_scroll, Some(true), "type beats \"*\"");
    assert_eq!(c.pointer_speed, Some(0.1), "unset in the type: from \"*\"");

    let mouse = DeviceKind {
        pointer: true,
        ..Default::default()
    };
    assert_eq!(config.input_for("Mouse", mouse).natural_scroll, Some(false));

    let touch = DeviceKind {
        touch: true,
        ..Default::default()
    };
    assert_eq!(
        config.input_for("Screen", touch).map_to_output.as_deref(),
        Some("DP-1")
    );
    let tablet = DeviceKind {
        tablet: true,
        ..Default::default()
    };
    assert_eq!(
        config.input_for("Wacom Pen", tablet).map_to_output.as_deref(),
        Some("HDMI-A-1"),
        "the device's own section beats everything"
    );
    assert_eq!(config.input_for("Nothing", DeviceKind::default()).tap, None);
}

#[test]
fn touch_goes_to_the_surface_under_it_and_focuses_its_window() {
    let (mut f, id) = setup("");
    let a = f.map_window(id, (400, 300), RED, Some("a"));
    let _b = f.map_window(id, (400, 300), RED, Some("b"));
    // Move a out from under b.
    let wa = f.windows()[0].clone();
    wa.data().loc = (100, 100).into();
    assert_ne!(f.focused(), Some(wa.clone()));

    {
        let s = f.hecate();
        s.touch_down(slot(0), (150., 120.).into(), time());
        s.touch_frame();
    }
    f.double_roundtrip(id);
    assert_eq!(f.focused(), Some(wa.clone()), "touch focuses the window");
    let events = touch_events(&mut f, id);
    match &events[..] {
        [TouchEvent::Down { surface, id: 0, x, y }, TouchEvent::Frame] => {
            assert_eq!(surface, &a);
            assert_eq!((*x, *y), (50., 20.), "surface-local position");
        }
        other => panic!("unexpected touch events: {other:?}"),
    }

    {
        let s = f.hecate();
        s.touch_motion(slot(0), (160., 130.).into(), time());
        s.touch_up(slot(0), time());
        s.touch_frame();
    }
    f.double_roundtrip(id);
    assert_eq!(
        touch_events(&mut f, id),
        vec![
            TouchEvent::Motion {
                id: 0,
                x: 60.,
                y: 30.
            },
            TouchEvent::Up { id: 0 },
            TouchEvent::Frame
        ]
    );
}

#[test]
fn touch_on_the_desktop_sends_nothing() {
    let (mut f, id) = setup("");
    f.map_window(id, (400, 300), RED, Some("a"));
    {
        let s = f.hecate();
        s.touch_down(slot(0), (5., 5.).into(), time());
        s.touch_up(slot(0), time());
        s.touch_frame();
    }
    f.double_roundtrip(id);
    let events = touch_events(&mut f, id);
    assert!(
        !events.iter().any(|e| matches!(e, TouchEvent::Down { .. })),
        "{events:?}"
    );
}

fn active_index(f: &mut Fixture) -> usize {
    let o = f.hecate().hecate.wm.outputs().next().unwrap().clone();
    let wm = &f.hecate().hecate.wm;
    let id = wm.active_workspace_id(&o);
    // 1-based.
    wm.workspace_index(id).unwrap() - 1
}

fn swipe(f: &mut Fixture, fingers: u32, dx: f64, dy: f64) {
    let s = f.hecate();
    s.swipe_begin(fingers, time());
    for _ in 0..10 {
        s.swipe_update((dx / 10., dy / 10.).into(), time());
    }
    s.swipe_end(false, time());
}

#[test]
fn three_finger_swipe_switches_workspaces() {
    let (mut f, _) = setup("");
    assert_eq!(active_index(&mut f), 0);
    swipe(&mut f, 3, -250., 10.);
    assert_eq!(active_index(&mut f), 1, "fingers left: next workspace");
    swipe(&mut f, 3, 250., 0.);
    assert_eq!(active_index(&mut f), 0, "fingers right: previous");
    swipe(&mut f, 3, -100., 0.);
    assert_eq!(active_index(&mut f), 0, "too short");
    swipe(&mut f, 3, -250., -400.);
    assert_eq!(active_index(&mut f), 0, "mostly vertical");
    swipe(&mut f, 4, -250., 0.);
    assert_eq!(active_index(&mut f), 0, "other finger counts go to clients");
    assert!(f.hecate().hecate.gestures_idle());

    let (mut f, _) = setup("[gestures]\nworkspace_swipe_invert = true\nworkspace_swipe_fingers = 4");
    swipe(&mut f, 4, 250., 0.);
    assert_eq!(active_index(&mut f), 1, "inverted, with four fingers");
    swipe(&mut f, 3, 250., 0.);
    assert_eq!(active_index(&mut f), 1, "three fingers no longer ours");
}

#[test]
fn absolute_devices_map_to_their_output() {
    let mut f = Fixture::with_toml(
        "[input.\"type:tablet\"]\nmap_to_output = \"headless-2\"\n[input.\"Odd Screen\"]\nmap_to_output = \"nope\"",
    );
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1280, 720));
    let tablet = DeviceKind {
        tablet: true,
        ..Default::default()
    };
    let touch = DeviceKind {
        touch: true,
        ..Default::default()
    };
    let s = f.hecate();
    let name = |o: Option<smithay::output::Output>| o.map(|o| o.name());
    assert_eq!(
        name(s.absolute_output("Pen", tablet)).as_deref(),
        Some("headless-2")
    );
    let focused = s.hecate.wm.focused_output.clone().map(|o| o.name());
    assert_eq!(
        name(s.absolute_output("Screen", touch)),
        focused,
        "unmapped: focused output"
    );
    assert_eq!(
        name(s.absolute_output("Odd Screen", touch)),
        focused,
        "unknown output name: focused output"
    );
}
