//! wlr-output-management as kanshi / wlr-randr would use it.

use smithay::reexports::wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::AdaptiveSyncState;
use wayland_client::protocol::wl_output::Transform;

use super::client::ConfResult;
use super::fixture::Fixture;

fn setup() -> (Fixture, super::client::ClientId) {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1920, 1080));
    let id = f.add_client();
    f.double_roundtrip(id);
    (f, id)
}

#[test]
fn heads_describe_outputs() {
    let (mut f, id) = setup();
    let st = &f.client(id).state;
    assert!(st.output_manager.is_some());
    assert!(st.output_serial.is_some(), "done sent");
    let (_, h1) = st.head("headless-1");
    let (_, h2) = st.head("headless-2");
    assert!(h1.enabled && h2.enabled);
    assert_eq!(h1.modes.len(), 3);
    let current = st.head_mode(h1.current_mode.as_ref().expect("current mode"));
    assert_eq!(current.size, (1920, 1080));
    assert_eq!(current.refresh, 60_000);
    assert!(current.preferred);
    assert_eq!(h1.position, (0, 0));
    assert_eq!(h2.position, (1920, 0), "second output placed to the right");
    assert_eq!(h1.scale, 1.0);
}

#[test]
fn apply_changes_scale_position_mode_and_records_overrides() {
    let (mut f, id) = setup();
    let st = &mut f.client(id).state;
    let (head, h) = st.head("headless-1");
    let mode_720 = h
        .modes
        .iter()
        .find(|m| st.head_mode(m).size == (1280, 720))
        .unwrap()
        .clone();
    let conf = st.create_output_configuration();
    let ch = conf.enable_head(&head, &st.qh, ());
    ch.set_mode(&mode_720);
    ch.set_scale(2.0);
    ch.set_position(100, 50);
    ch.set_transform(Transform::_90);
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Succeeded);

    let o = f.hecate().hecate.wm.output_by_name("headless-1").unwrap().clone();
    assert_eq!(o.current_mode().unwrap().size, (1280, 720).into());
    assert_eq!(o.current_scale().fractional_scale(), 2.0);
    assert_eq!(o.current_location(), (100, 50).into());
    assert_eq!(o.current_transform(), smithay::utils::Transform::_90);
    let overrides: Vec<String> = f
        .hecate()
        .hecate
        .overrides
        .iter()
        .map(|(p, _)| p.clone())
        .collect();
    assert!(
        overrides.contains(&"outputs.headless-1.scale".to_string()),
        "{overrides:?}"
    );

    // Clients hear about it.
    f.double_roundtrip(id);
    let st = &f.client(id).state;
    let (_, h) = st.head("headless-1");
    assert_eq!(h.scale, 2.0);
    assert_eq!(h.position, (100, 50));
    assert_eq!(h.transform, Transform::_90 as i32);
    assert_eq!(st.head_mode(h.current_mode.as_ref().unwrap()).size, (1280, 720));
}

#[test]
fn disable_and_reenable_output() {
    let (mut f, id) = setup();
    let st = &mut f.client(id).state;
    let (head2, _) = st.head("headless-2");
    let conf = st.create_output_configuration();
    conf.disable_head(&head2);
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Succeeded);
    assert!(
        f.hecate().hecate.wm.output_by_name("headless-2").is_none(),
        "output removed"
    );
    f.double_roundtrip(id);
    assert!(
        !f.client(id).state.head("headless-2").1.enabled,
        "head still listed, disabled"
    );

    let st = &mut f.client(id).state;
    let (head2, _) = st.head("headless-2");
    let conf = st.create_output_configuration();
    conf.enable_head(&head2, &st.qh, ());
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Succeeded);
    assert!(
        f.hecate().hecate.wm.output_by_name("headless-2").is_some(),
        "output back"
    );
    f.double_roundtrip(id);
    assert!(f.client(id).state.head("headless-2").1.enabled);
}

#[test]
fn disabling_everything_fails_and_test_does_not_apply() {
    let (mut f, id) = setup();
    let st = &mut f.client(id).state;
    let (h1, _) = st.head("headless-1");
    let (h2, _) = st.head("headless-2");
    let conf = st.create_output_configuration();
    conf.disable_head(&h1);
    conf.disable_head(&h2);
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Failed);
    assert_eq!(f.hecate().hecate.wm.outputs().count(), 2);

    let st = &mut f.client(id).state;
    let conf = st.create_output_configuration();
    let ch = conf.enable_head(&h1, &st.qh, ());
    ch.set_scale(1.5);
    conf.test();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Succeeded);
    let o = f.hecate().hecate.wm.output_by_name("headless-1").unwrap().clone();
    assert_eq!(o.current_scale().fractional_scale(), 1.0, "test must not apply");
    assert!(f.hecate().hecate.overrides.is_empty());
}

#[test]
fn unknown_custom_mode_fails_and_stale_serial_is_cancelled() {
    let (mut f, id) = setup();
    let st = &mut f.client(id).state;
    let (h1, _) = st.head("headless-1");
    let conf = st.create_output_configuration();
    let ch = conf.enable_head(&h1, &st.qh, ());
    ch.set_custom_mode(1234, 567, 60_000);
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Failed);

    // A configuration created before outputs changed gets cancelled.
    let conf = f.client(id).state.create_output_configuration();
    f.hecate()
        .set_override("outputs.headless-2.scale", serde_json::json!(2.0))
        .unwrap();
    f.double_roundtrip(id);
    let st = &mut f.client(id).state;
    assert_eq!(st.output_confs[&conf], ConfResult::Cancelled);
}

#[test]
fn adaptive_sync_toggle_sets_config() {
    let (mut f, id) = setup();
    let st = &mut f.client(id).state;
    let (h1, _) = st.head("headless-1");
    let conf = st.create_output_configuration();
    let ch = conf.enable_head(&h1, &st.qh, ());
    ch.set_adaptive_sync(AdaptiveSyncState::Enabled);
    conf.apply();
    f.double_roundtrip(id);
    assert_eq!(f.client(id).state.output_confs[&conf], ConfResult::Succeeded);
    assert_eq!(
        f.hecate().hecate.config.outputs["headless-1"].adaptive_sync,
        Some(crate::config::Vrr::On)
    );
    f.double_roundtrip(id);
    assert!(f.client(id).state.head("headless-1").1.adaptive_sync);
}
