//! xwayland-satellite on demand, with a fake satellite that records how it was started.

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::fixture::Fixture;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hecate-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fake satellite: answers the capability probe, otherwise logs its arguments and exits.
fn fake_satellite(dir: &std::path::Path, listenfd: bool) -> (PathBuf, PathBuf) {
    let log = dir.join("args.log");
    let script = dir.join("xwayland-satellite");
    let probe = if listenfd { "exit 0" } else { "exit 1" };
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ \"$2\" = --test-listenfd-support ]; then {probe}; fi\necho \"$@\" >> {}\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (script, log)
}

fn wait_for(f: &mut Fixture, what: &str, mut cond: impl FnMut(&mut Fixture) -> bool) {
    let start = Instant::now();
    while !cond(f) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        f.dispatch();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn satellite_starts_on_first_x11_client_and_restarts() {
    let dir = temp_dir("xwl");
    let (script, log) = fake_satellite(&dir, true);
    let mut f = Fixture::with_toml(&format!("[xwayland]\npath = \"{}\"", script.display()));
    f.hecate().setup_xwayland_in(&dir);

    let display = f
        .hecate()
        .hecate
        .satellite
        .as_ref()
        .expect("X11 set up")
        .display_name()
        .to_string();
    assert_eq!(display, ":0");
    assert!(dir.join(".X0-lock").exists());
    let socket = dir.join(".X11-unix/X0");
    assert!(socket.exists());
    assert_eq!(
        f.hecate().hecate.satellite.as_ref().unwrap().spawn_count,
        0,
        "nothing until a client"
    );

    // An X11 client connects: satellite is started with our sockets.
    let _c1 = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    wait_for(&mut f, "first start", |f| {
        f.hecate().hecate.satellite.as_ref().unwrap().spawn_count == 1
    });
    wait_for(&mut f, "satellite args", |_| {
        std::fs::read_to_string(&log).is_ok_and(|s| s.contains("-listenfd"))
    });
    let args = std::fs::read_to_string(&log).unwrap();
    assert!(args.starts_with(":0 -listenfd "), "{args}");
    assert_eq!(args.matches("-listenfd").count(), 2, "unix and abstract sockets");

    // The fake exits right away; the next client starts it again.
    std::thread::sleep(Duration::from_millis(200));
    for _ in 0..20 {
        f.dispatch();
    }
    let _c2 = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    wait_for(&mut f, "restart", |f| {
        f.hecate().hecate.satellite.as_ref().unwrap().spawn_count >= 2
    });

    // Disabling X11 closes the display.
    f.hecate().teardown_xwayland();
    assert!(f.hecate().hecate.satellite.is_none());
    assert!(!socket.exists(), "socket removed");
    assert!(!dir.join(".X0-lock").exists(), "lock removed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_satellite_without_listenfd_disables_x11() {
    let dir = temp_dir("xwl-old");
    let (script, _) = fake_satellite(&dir, false);
    let mut f = Fixture::with_toml(&format!("[xwayland]\npath = \"{}\"", script.display()));
    f.hecate().setup_xwayland_in(&dir);
    assert!(f.hecate().hecate.satellite.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn second_display_is_picked_when_taken() {
    let dir = temp_dir("xwl-taken");
    std::fs::write(dir.join(".X0-lock"), "1\n").unwrap();
    let (script, _) = fake_satellite(&dir, true);
    let mut f = Fixture::with_toml(&format!("[xwayland]\npath = \"{}\"", script.display()));
    f.hecate().setup_xwayland_in(&dir);
    assert_eq!(f.hecate().hecate.satellite.as_ref().unwrap().display_name(), ":1");
    f.hecate().teardown_xwayland();
    let _ = std::fs::remove_dir_all(&dir);
}
