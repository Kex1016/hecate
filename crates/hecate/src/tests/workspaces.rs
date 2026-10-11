use super::fixture::Fixture;
use crate::config::MAX_WORKSPACES;
use crate::config::action::{Action, OutputRef, WorkspaceRef};
use crate::wm::WindowExt;

const RED: [u8; 4] = [255, 0, 0, 255];

#[test]
fn fixed_count_and_names() {
    let mut f = Fixture::with_toml("[workspaces]\ncount = 4\nnames = [\"web\", \"code\"]");
    f.add_output("headless-1", (1920, 1080));
    let wm = &f.hecate().hecate.wm;
    assert_eq!(wm.workspaces.len(), 4);
    assert_eq!(wm.workspaces[0].name.as_deref(), Some("web"));
    assert_eq!(wm.workspaces[1].name.as_deref(), Some("code"));
    assert_eq!(wm.workspaces[2].name, None);
}

#[test]
fn switching_hides_windows_and_move_to_workspace() {
    let mut f = Fixture::new();
    let output = f.add_output("headless-1", (1920, 1080));
    let id = f.add_client();
    f.map_window(id, (300, 300), RED, Some("a"));
    let w = f.windows()[0].clone();

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    assert!(f.hecate().hecate.wm.windows_on_output(&output).is_empty());
    assert_eq!(f.focused(), None, "nothing to focus on an empty workspace");

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(1)), None);
    assert_eq!(f.hecate().hecate.wm.windows_on_output(&output), vec![w.clone()]);
    assert_eq!(f.focused(), Some(w.clone()), "focus comes back");

    f.hecate()
        .do_action(Action::MoveToWorkspace(WorkspaceRef::Index(3)), None);
    let ws3 = f.hecate().hecate.wm.workspaces[2].id;
    assert_eq!(w.data().workspace, ws3);
    assert!(f.hecate().hecate.wm.windows_on_output(&output).is_empty());
}

#[test]
fn next_prev_wrap_in_fixed_mode() {
    let mut f = Fixture::with_toml("[workspaces]\ncount = 3");
    let output = f.add_output("headless-1", (1920, 1080));
    let ids: Vec<u64> = f.hecate().hecate.wm.workspaces.iter().map(|w| w.id).collect();
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Named("prev".into())), None);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ids[2]);
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Named("next".into())), None);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ids[0]);
}

#[test]
fn back_and_forth() {
    let mut f = Fixture::with_toml("[workspaces]\nauto_back_and_forth = true");
    let output = f.add_output("headless-1", (1920, 1080));
    let ids: Vec<u64> = f.hecate().hecate.wm.workspaces.iter().map(|w| w.id).collect();
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(3)), None);
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(3)), None);
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), ids[0]);
}

#[test]
fn auto_mode_keeps_one_empty_at_the_end() {
    let mut f = Fixture::with_toml("[workspaces]\nkind = \"auto\"");
    let output = f.add_output("headless-1", (1920, 1080));
    assert_eq!(
        f.hecate().hecate.wm.workspaces.len(),
        1,
        "starts with one empty workspace"
    );

    let id = f.add_client();
    f.map_window(id, (300, 300), RED, None);
    assert_eq!(
        f.hecate().hecate.wm.workspaces.len(),
        2,
        "an empty one appears after the used one"
    );

    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Named("next".into())), None);
    let second = f.hecate().hecate.wm.workspaces[1].id;
    assert_eq!(f.hecate().hecate.wm.active_workspace_id(&output), second);
    assert_eq!(f.hecate().hecate.wm.workspaces.len(), 2);

    // Moving the window here empties workspace 1, which then vanishes.
    let w = f.windows()[0].clone();
    f.hecate()
        .do_action(Action::MoveToWorkspace(WorkspaceRef::Index(2)), Some(w.clone()));
    let wm = &f.hecate().hecate.wm;
    assert_eq!(
        wm.workspaces.len(),
        2,
        "[window ws, empty]: {:?}",
        wm.workspaces.iter().map(|w| w.windows.len()).collect::<Vec<_>>()
    );
    assert!(!wm.workspaces[0].is_empty());
    assert!(wm.workspaces[1].is_empty());
}

#[test]
fn auto_mode_caps_at_max() {
    let mut f = Fixture::with_toml("[workspaces]\nkind = \"auto\"");
    let output = f.add_output("headless-1", (1920, 1080));
    let wm = &mut f.hecate().hecate.wm;
    assert!(
        wm.resolve_workspace(&output, &WorkspaceRef::Index(MAX_WORKSPACES))
            .is_some()
    );
    assert!(
        wm.resolve_workspace(&output, &WorkspaceRef::Index(MAX_WORKSPACES + 1))
            .is_none()
    );
    assert!(wm.workspaces.len() <= MAX_WORKSPACES as usize);
}

#[test]
fn global_scope_switches_all_outputs() {
    let mut f = Fixture::new();
    let a = f.add_output("headless-1", (1920, 1080));
    let b = f.add_output("headless-2", (1280, 720));
    assert_eq!(f.hecate().hecate.wm.workspaces.len(), 9, "one shared set");
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(4)), None);
    let wm = &f.hecate().hecate.wm;
    assert_eq!(wm.active_workspace_id(&a), wm.active_workspace_id(&b));
    // Second output sits to the right of the first.
    assert_eq!(b.current_location(), (1920, 0).into());
}

#[test]
fn per_output_scope_is_independent() {
    let mut f = Fixture::with_toml("[workspaces]\nscope = \"per_output\"\ncount = 3");
    let a = f.add_output("headless-1", (1920, 1080));
    let b = f.add_output("headless-2", (1280, 720));
    assert_eq!(f.hecate().hecate.wm.workspaces.len(), 6, "3 per output");
    f.hecate().hecate.wm.focused_output = Some(a.clone());
    f.hecate()
        .do_action(Action::Workspace(WorkspaceRef::Index(2)), None);
    let wm = &f.hecate().hecate.wm;
    assert_eq!(wm.workspace_index(wm.active_workspace_id(&a)), Some(2));
    assert_eq!(
        wm.workspace_index(wm.active_workspace_id(&b)),
        Some(1),
        "other output unchanged"
    );
}

#[test]
fn move_to_output_per_output_scope() {
    let mut f = Fixture::with_toml("[workspaces]\nscope = \"per_output\"");
    let a = f.add_output("headless-1", (1920, 1080));
    let b = f.add_output("headless-2", (1280, 720));
    f.hecate().hecate.wm.focused_output = Some(a.clone());
    let id = f.add_client();
    f.map_window(id, (300, 300), RED, None);
    let w = f.windows()[0].clone();
    f.hecate()
        .do_action(Action::MoveToOutput(OutputRef("right".into())), None);
    let wm = &f.hecate().hecate.wm;
    assert_eq!(wm.output_for_window(&w), Some(&b));
    assert_eq!(w.data().workspace, wm.active_workspace_id(&b));
    let _ = w.wid();
}

#[test]
fn scope_change_on_reload_keeps_windows() {
    let mut f = Fixture::new();
    f.add_output("headless-1", (1920, 1080));
    f.add_output("headless-2", (1280, 720));
    let id = f.add_client();
    f.map_window(id, (300, 300), RED, None);
    let cfg = crate::config::load::from_toml_str("[workspaces]\nscope = \"per_output\"").unwrap();
    f.hecate().apply_config(cfg);
    f.dispatch();
    assert_eq!(f.hecate().hecate.wm.workspaces.len(), 18);
    let w = f.windows()[0].clone();
    assert!(
        f.hecate().hecate.wm.workspace_of(&w).is_some(),
        "window still belongs to a workspace"
    );
}
