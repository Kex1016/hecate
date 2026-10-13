//! wlr-gamma-control: what a night-light tool would do.

use super::client::send_gamma;
use super::fixture::Fixture;

fn ramp(scale: f64) -> Vec<u16> {
    (0..3)
        .flat_map(|_| (0..256u32).map(move |i| ((i * 0xFFFF / 255) as f64 * scale) as u16))
        .collect()
}

#[test]
fn set_gamma_reaches_backend_and_resets_on_destroy() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    let wl_output = f.client(id).output("headless-1");
    let control = f.client(id).state.get_gamma_control(&wl_output);
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.gamma_status(&control), (Some(256), false));

    let warm = ramp(0.8);
    send_gamma(&control, &warm);
    f.double_roundtrip(id);
    let set = f.hecate().backend.headless().gamma.get("headless-1").cloned();
    assert_eq!(set.as_deref(), Some(&warm[..]), "ramp applied to the output");

    control.destroy();
    f.double_roundtrip(id);
    assert!(
        !f.hecate().backend.headless().gamma.contains_key("headless-1"),
        "gamma reset when the client lets go"
    );
}

#[test]
fn second_control_on_same_output_fails() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let a = f.add_client();
    let b = f.add_client();
    f.double_roundtrip(a);
    f.double_roundtrip(b);
    let oa = f.client(a).output("headless-1");
    let ob = f.client(b).output("headless-1");
    let ca = f.client(a).state.get_gamma_control(&oa);
    f.double_roundtrip(a);
    let cb = f.client(b).state.get_gamma_control(&ob);
    f.double_roundtrip(b);
    assert!(!f.client(a).state.gamma_status(&ca).1);
    assert!(
        f.client(b).state.gamma_status(&cb).1,
        "only one controller per output"
    );

    // Once the first one goes away, another client can take over.
    ca.destroy();
    f.double_roundtrip(a);
    let cb2 = f.client(b).state.get_gamma_control(&ob);
    f.double_roundtrip(b);
    assert_eq!(f.client(b).state.gamma_status(&cb2), (Some(256), false));
}

#[test]
fn wrong_size_fails_and_resets() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    let o = f.client(id).output("headless-1");
    let control = f.client(id).state.get_gamma_control(&o);
    f.double_roundtrip(id);
    send_gamma(&control, &ramp(0.5));
    f.double_roundtrip(id);
    assert!(f.hecate().backend.headless().gamma.contains_key("headless-1"));

    send_gamma(&control, &[0u16; 10]);
    f.double_roundtrip(id);
    assert!(f.client(id).state.gamma_status(&control).1, "failed sent");
    assert!(
        !f.hecate().backend.headless().gamma.contains_key("headless-1"),
        "gamma reset"
    );
}
