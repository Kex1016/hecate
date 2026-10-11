//! ext-workspace-v1 as a workspace bar would see it.

use super::fixture::Fixture;
use crate::config::action::{Action, WorkspaceRef};
use crate::wm::WindowExt;

const RED: [u8; 4] = [255, 0, 0, 255];

#[test]
fn global_scope_single_group_on_all_outputs() {
    let mut f = Fixture::with_toml("[workspaces]\ncount = 3\nnames = [\"web\"]");
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1280, 720));
    let bar = f.add_client();
    f.double_roundtrip(bar);

    let st = &f.client(bar).state;
    assert!(st.workspace_manager.is_some(), "ext-workspace advertised");
    assert!(st.ws_done_count >= 1, "done sent");
    let groups = st.live_ws_groups();
    assert_eq!(groups.len(), 1, "one group in global scope");
    assert_eq!(groups[0].1.outputs.len(), 2, "group spans both outputs");
    assert_eq!(groups[0].1.workspaces.len(), 3);

    let ws = st.live_workspaces();
    assert_eq!(ws.len(), 3);
    let web = st.workspace_named("web").expect("named workspace");
    assert_eq!(
        web.1.id.as_deref(),
        Some("web"),
        "named workspaces have a stable id"
    );
    assert_eq!(web.1.coordinates, vec![0]);
    assert!(web.1.active());
    let two = st
        .workspace_named("2")
        .expect("unnamed workspace labelled by index");
    assert_eq!(two.1.id, None);
    assert_eq!(two.1.coordinates, vec![1]);
    assert!(!two.1.active());
}

#[test]
fn per_output_scope_has_group_per_output() {
    let mut f = Fixture::with_toml("[workspaces]\nscope = \"per_output\"\ncount = 2");
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1280, 720));
    let bar = f.add_client();
    f.double_roundtrip(bar);
    let st = &f.client(bar).state;
    let groups = st.live_ws_groups();
    assert_eq!(groups.len(), 2);
    for (_, g) in &groups {
        assert_eq!(g.outputs.len(), 1);
        assert_eq!(g.workspaces.len(), 2);
    }
    let active = st.live_workspaces().iter().filter(|(_, w)| w.active()).count();
    assert_eq!(active, 2, "one active workspace per output");
}

#[test]
fn switching_updates_state_and_activate_request_switches() {
    let mut f = Fixture::with_toml("[workspaces]\ncount = 3");
    let output = f.add_output("headless-1", (1920, 1080));
    let bar = f.add_client();
    f.double_roundtrip(bar);

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    f.double_roundtrip(bar);
    let st = &f.client(bar).state;
    assert!(!st.workspace_named("1").unwrap().1.active());
    assert!(st.workspace_named("2").unwrap().1.active());

    // The bar asks for workspace 3; nothing happens until commit.
    let (h3, _) = f.client(bar).state.workspace_named("3").unwrap();
    h3.activate();
    f.double_roundtrip(bar);
    let ids: Vec<u64> = f.hecate().hecate.wm.workspaces.iter().map(|w| w.id).collect();
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ids[1]);
    f.client(bar).state.workspace_manager.clone().unwrap().commit();
    f.double_roundtrip(bar);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ids[2]);
    f.double_roundtrip(bar);
    assert!(f.client(bar).state.workspace_named("3").unwrap().1.active());
}

#[test]
fn auto_workspaces_appear_and_disappear() {
    let mut f = Fixture::with_toml("[workspaces]\nkind = \"auto\"");
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let bar = f.add_client();
    f.double_roundtrip(bar);
    let before = f.client(bar).state.live_workspaces().len();

    f.map_window(app, (300, 300), RED, Some("a"));
    f.hecate().do_action(
        Action::MoveToWorkspace(WorkspaceRef::Index(before as u32 + 1)),
        None,
    );
    f.double_roundtrip(bar);
    let after = f.client(bar).state.live_workspaces().len();
    assert!(after > before, "new workspace announced ({before} -> {after})");

    // Bring the window back; the extra workspace empties and goes away.
    let w = f.windows()[0].clone();
    let first = f.hecate().hecate.wm.workspaces[0].id;
    f.hecate().hecate.wm.move_window_to_workspace(&w, first);
    assert_eq!(w.data().workspace, first);
    for _ in 0..3 {
        f.double_roundtrip(bar);
    }
    let fin = f.client(bar).state.live_workspaces().len();
    let wm_count = f.hecate().hecate.wm.workspaces.len();
    assert_eq!(fin, wm_count, "client list matches the compositor");
    assert!(fin < after, "removed sent for the emptied workspace");
}

#[test]
fn urgent_window_marks_workspace() {
    let mut f = Fixture::with_toml("[workspaces]\ncount = 2");
    f.add_output("headless-1", (1920, 1080));
    let app = f.add_client();
    let bar = f.add_client();
    f.map_window(app, (300, 300), RED, Some("a"));
    let w = f.windows()[0].clone();
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    w.data().urgent = true;
    f.double_roundtrip(bar);
    assert!(f.client(bar).state.workspace_named("1").unwrap().1.urgent());
}
