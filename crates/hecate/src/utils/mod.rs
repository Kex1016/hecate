//! Small helpers.

use std::collections::BTreeMap;
use std::process::{Command, Stdio};
use std::sync::RwLock;
use std::time::Duration;

use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Transform;
use smithay::wayland::compositor::{SurfaceData, get_parent, send_surface_state};
use smithay::wayland::fractional_scale::with_fractional_scale;

/// Environment for spawned children: `(name, value)`, value `None` removes the variable.
static CHILD_ENV: RwLock<Vec<(String, Option<String>)>> = RwLock::new(Vec::new());

/// Set an environment variable for children spawned from now on.
pub fn set_child_env(name: &str, value: Option<&str>) {
    let mut env = CHILD_ENV.write().unwrap();
    env.retain(|(n, _)| n != name);
    env.push((name.to_string(), value.map(String::from)));
}

pub fn child_env() -> Vec<(String, Option<String>)> {
    CHILD_ENV.read().unwrap().clone()
}

/// Spawn a command detached from the compositor. Waits in a thread so it doesn't zombie.
pub fn spawn(argv: &[String], wayland_display: Option<&str>, extra_env: &BTreeMap<String, String>) {
    let Some((cmd, args)) = argv.split_first() else {
        return;
    };
    let cmd = expand_home(cmd);
    let args = args.to_vec();
    let wayland_display = wayland_display.map(String::from);
    let extra_env = extra_env.clone();
    let res = std::thread::Builder::new().name("spawner".into()).spawn(move || {
        let mut command = Command::new(&cmd);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env_remove("NOTIFY_SOCKET");
        if let Some(d) = &wayland_display {
            command.env("WAYLAND_DISPLAY", d);
        }
        for (k, v) in child_env() {
            match v {
                Some(v) => command.env(k, v),
                None => command.env_remove(k),
            };
        }
        for (k, v) in &extra_env {
            command.env(k, v);
        }
        // SAFETY: sigprocmask is async-signal-safe.
        unsafe {
            std::os::unix::process::CommandExt::pre_exec(&mut command, || {
                unblock_signals();
                Ok(())
            });
        }
        match command.spawn() {
            Ok(mut child) => {
                let _ = child.wait();
            }
            Err(err) => tracing::warn!("error spawning {cmd:?}: {err}"),
        }
    });
    if let Err(err) = res {
        tracing::warn!("error starting spawner thread: {err}");
    }
}

/// Children must not inherit the signals we block for calloop (SIGHUP for reload).
pub fn unblock_signals() {
    // SAFETY: plain sigprocmask with a valid, empty set.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigprocmask(libc::SIG_SETMASK, &set, std::ptr::null_mut());
    }
}

pub fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return format!("{}/{rest}", home.to_string_lossy());
    }
    path.to_string()
}

/// Resolve the terminal command: config, `$TERMINAL`, then the first of a few known ones.
pub fn terminal_command(configured: Option<&Vec<String>>) -> Vec<String> {
    if let Some(t) = configured
        && !t.is_empty()
    {
        return t.clone();
    }
    if let Ok(t) = std::env::var("TERMINAL")
        && !t.is_empty()
    {
        return vec![t];
    }
    for candidate in [
        "foot",
        "kitty",
        "alacritty",
        "wezterm",
        "konsole",
        "gnome-terminal",
    ] {
        if which(candidate) {
            return vec![candidate.to_string()];
        }
    }
    vec!["foot".to_string()]
}

pub fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(bin).is_file()))
        .unwrap_or(false)
}

pub fn root_surface(surface: &WlSurface) -> WlSurface {
    let mut root = surface.clone();
    while let Some(parent) = get_parent(&root) {
        root = parent;
    }
    root
}

/// The surface has a buffer attached (has committed content).
pub fn is_mapped(surface: &WlSurface) -> bool {
    with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false)
}

pub fn send_scale_transform(
    surface: &WlSurface,
    data: &SurfaceData,
    scale: output::Scale,
    transform: Transform,
) {
    send_surface_state(surface, data, scale.integer_scale(), transform);
    with_fractional_scale(data, |fractional| {
        fractional.set_preferred_scale(scale.fractional_scale());
    });
}

pub fn monotonic_time() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime with a valid pointer.
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_prefers_config() {
        let t = terminal_command(Some(&vec!["wezterm".into(), "start".into()]));
        assert_eq!(t, vec!["wezterm", "start"]);
    }

    #[test]
    fn child_env_overrides() {
        set_child_env("HECATE_TEST_VAR", Some("1"));
        set_child_env("HECATE_TEST_VAR", Some("2"));
        let env = child_env();
        let vals: Vec<_> = env.iter().filter(|(k, _)| k == "HECATE_TEST_VAR").collect();
        assert_eq!(vals.len(), 1);
        assert_eq!(vals[0].1.as_deref(), Some("2"));
    }
}
