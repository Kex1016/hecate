//! What the server may do, and where it finds things.
//!
//! Everything that only looks is on. Everything that changes something is off until it is
//! allowed, on the command line (`--allow tint,notify`) or in `$XDG_CONFIG_HOME/hecate/mcp.toml`:
//!
//! ```toml
//! allow = ["windows", "tint", "notify"]   # or ["all"]
//! screenshots = true
//! notifications_file = "~/.cache/quickshell/notifications/notifications.json"
//! grim = "grim"
//! notify_send = "notify-send"
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A kind of change the server can be allowed to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    /// Focus windows, move them between workspaces, switch workspace.
    Windows,
    /// Set and clear border tints.
    Tint,
    /// Send desktop notifications.
    Notify,
}

impl Permission {
    pub const ALL: [Permission; 3] = [Permission::Windows, Permission::Tint, Permission::Notify];

    pub fn name(self) -> &'static str {
        match self {
            Permission::Windows => "windows",
            Permission::Tint => "tint",
            Permission::Notify => "notify",
        }
    }

    /// One name, or `all`.
    pub fn parse(name: &str) -> Result<Vec<Permission>, String> {
        let name = name.trim();
        if name == "all" {
            return Ok(Permission::ALL.to_vec());
        }
        Permission::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| vec![p])
            .ok_or_else(|| format!("unknown permission {name:?}: windows, tint, notify or all"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub allow: BTreeSet<Permission>,
    pub screenshots: bool,
    pub notifications_file: PathBuf,
    pub grim: String,
    pub notify_send: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            allow: BTreeSet::new(),
            screenshots: true,
            notifications_file: default_notifications_file(),
            grim: "grim".into(),
            notify_send: "notify-send".into(),
        }
    }
}

/// The file as written; everything optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    allow: Vec<String>,
    screenshots: Option<bool>,
    notifications_file: Option<String>,
    grim: Option<String>,
    notify_send: Option<String>,
}

impl Config {
    pub fn allows(&self, p: Permission) -> bool {
        self.allow.contains(&p)
    }

    /// Lay a config file's settings over this one.
    pub fn apply_toml(&mut self, text: &str) -> Result<(), String> {
        let file: File = toml::from_str(text).map_err(|e| e.to_string())?;
        for name in &file.allow {
            self.allow.extend(Permission::parse(name)?);
        }
        if let Some(s) = file.screenshots {
            self.screenshots = s;
        }
        if let Some(p) = file.notifications_file {
            self.notifications_file = expand_home(&p);
        }
        if let Some(g) = file.grim {
            self.grim = g;
        }
        if let Some(n) = file.notify_send {
            self.notify_send = n;
        }
        Ok(())
    }
}

/// `$XDG_CONFIG_HOME/hecate/mcp.toml`.
pub fn default_config_file() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config").map(|d| d.join("hecate/mcp.toml"))
}

/// Where the bundled shell keeps its notification history.
fn default_notifications_file() -> PathBuf {
    xdg_dir("XDG_CACHE_HOME", ".cache")
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("quickshell/notifications/notifications.json")
}

fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => std::env::var_os("HOME").map(|h| Path::new(&h).join(fallback)),
    }
}

fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => PathBuf::from(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_that_changes_state_by_default() {
        let c = Config::default();
        for p in Permission::ALL {
            assert!(!c.allows(p), "{p:?}");
        }
        assert!(c.screenshots);
    }

    #[test]
    fn file_settings() {
        let mut c = Config::default();
        c.apply_toml(
            "allow = [\"tint\"]\nscreenshots = false\ngrim = \"/bin/grim\"\nnotifications_file = \"/n.json\"\n",
        )
        .unwrap();
        assert!(c.allows(Permission::Tint));
        assert!(!c.allows(Permission::Windows));
        assert!(!c.screenshots);
        assert_eq!(c.grim, "/bin/grim");
        assert_eq!(c.notifications_file, PathBuf::from("/n.json"));

        let mut c = Config::default();
        c.apply_toml("allow = [\"all\"]").unwrap();
        assert_eq!(c.allow.len(), 3);
        assert!(Config::default().apply_toml("allow = [\"root\"]").is_err());
        assert!(Config::default().apply_toml("alow = []").is_err());
    }
}
