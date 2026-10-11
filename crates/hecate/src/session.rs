//! Session integration: activation environment and the systemd user target.

use std::process::{Command, Stdio};

use crate::state::State;

const IMPORTED_VARS: &[&str] = &[
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "HECATE_SOCKET",
    "XCURSOR_THEME",
    "XCURSOR_SIZE",
];

fn run_quiet(argv: &[&str]) -> bool {
    Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl State {
    /// Export our environment to dbus/systemd activation and start `hecate-session.target`.
    pub fn start_session(&mut self) {
        if std::env::var_os("HECATE_NO_SESSION").is_some() {
            return;
        }
        let mut vars: Vec<String> = IMPORTED_VARS.iter().map(|s| s.to_string()).collect();
        vars.extend(self.hecate.config.environment.keys().cloned());
        vars.extend(self.hecate.config.session_variables.iter().cloned());
        let env = crate::utils::child_env();
        std::thread::spawn(move || {
            let assignments: Vec<String> = vars
                .iter()
                .filter_map(|k| {
                    env.iter()
                        .find(|(n, _)| n == k)
                        .and_then(|(_, v)| v.clone())
                        .or_else(|| std::env::var(k).ok())
                        .map(|v| format!("{k}={v}"))
                })
                .collect();
            let mut argv = vec!["dbus-update-activation-environment", "--systemd"];
            argv.extend(assignments.iter().map(String::as_str));
            if !run_quiet(&argv) {
                let mut argv = vec!["systemctl", "--user", "set-environment"];
                argv.extend(assignments.iter().map(String::as_str));
                run_quiet(&argv);
            }
            if run_quiet(&["systemctl", "--user", "cat", "hecate-session.target"]) {
                run_quiet(&["systemctl", "--user", "start", "hecate-session.target"]);
            }
        });
    }

    pub fn end_session(&mut self) {
        if std::env::var_os("HECATE_NO_SESSION").is_some() {
            return;
        }
        run_quiet(&["systemctl", "--user", "stop", "hecate-session.target"]);
    }
}
