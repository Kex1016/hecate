//! wlr-output-power-management (DPMS from wlopm / a shell's idle handling).

use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::client::zwlr_output_power_v1::Mode;

use super::fixture::Fixture;
use crate::config::action::Action;

#[test]
fn set_mode_powers_output_and_everyone_hears() {
    let mut f = Fixture::new();
    let output = f.add_output("headless-1", (1920, 1080));
    let a = f.add_client();
    let b = f.add_client();
    f.double_roundtrip(a);
    f.double_roundtrip(b);
    let oa = f.client(a).output("headless-1");
    let ob = f.client(b).output("headless-1");
    let ca = f.client(a).state.get_output_power(&oa);
    let cb = f.client(b).state.get_output_power(&ob);
    f.double_roundtrip(a);
    f.double_roundtrip(b);
    assert_eq!(
        f.client(a).state.power_status(&ca),
        (vec![Mode::On], false),
        "initial mode"
    );

    ca.set_mode(Mode::Off);
    f.double_roundtrip(a);
    f.double_roundtrip(b);
    assert!(f.hecate().hecate.output_state[&output].powered_off);
    assert_eq!(f.client(a).state.power_status(&ca).0.last(), Some(&Mode::Off));
    assert_eq!(
        f.client(b).state.power_status(&cb).0.last(),
        Some(&Mode::Off),
        "other clients notified"
    );

    // The compositor turning outputs on is reported too.
    f.hecate().do_action(Action::PowerOnOutputs, None);
    f.double_roundtrip(b);
    assert!(!f.hecate().hecate.output_state[&output].powered_off);
    assert_eq!(f.client(b).state.power_status(&cb).0.last(), Some(&Mode::On));
}

#[test]
fn power_off_action_is_reported() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    let o = f.client(id).output("headless-1");
    let c = f.client(id).state.get_output_power(&o);
    f.double_roundtrip(id);
    f.hecate().do_action(Action::PowerOffOutputs, None);
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.power_status(&c).0, vec![Mode::On, Mode::Off]);
    assert!(f.hecate().hecate.wake_outputs_on_input);
}

#[test]
fn removed_output_fails_control() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    let o = f.client(id).output("headless-2");
    let c = f.client(id).state.get_output_power(&o);
    f.double_roundtrip(id);
    f.hecate()
        .set_override("outputs.headless-2.enable", serde_json::json!(false))
        .unwrap();
    f.double_roundtrip(id);
    assert!(
        f.client(id).state.power_status(&c).1,
        "failed after the output went away"
    );
}
