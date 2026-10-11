//! `hecate check`: environment diagnostics.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    pub topic: &'static str,
    pub message: String,
}

fn f(level: Level, topic: &'static str, message: impl Into<String>) -> Finding {
    Finding {
        level,
        topic,
        message: message.into(),
    }
}

fn read_trim(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// GPUs and connectors from sysfs.
fn check_drm(out: &mut Vec<Finding>) {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        out.push(f(
            Level::Fail,
            "gpu",
            "no /sys/class/drm (no DRM drivers loaded?)",
        ));
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let cards: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("card") && !n.contains('-'))
        .collect();
    if cards.is_empty() {
        out.push(f(Level::Fail, "gpu", "no DRM cards found"));
    }
    for card in cards {
        let dev = PathBuf::from("/sys/class/drm").join(card).join("device");
        let driver = std::fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "unknown".into());
        let boot = read_trim(&dev.join("boot_vga")).is_some_and(|v| v == "1");
        out.push(f(
            Level::Ok,
            "gpu",
            format!("{card}: driver {driver}{}", if boot { " (boot GPU)" } else { "" }),
        ));
        if driver == "nvidia" {
            let modeset = read_trim(Path::new("/sys/module/nvidia_drm/parameters/modeset"));
            match modeset.as_deref() {
                Some("Y") => out.push(f(Level::Ok, "gpu", "nvidia-drm modeset enabled")),
                _ => out.push(f(
                    Level::Fail,
                    "gpu",
                    "nvidia-drm.modeset is not enabled (add nvidia-drm.modeset=1 to the kernel command line)",
                )),
            }
        }
        for conn in names.iter().filter(|n| n.starts_with(&format!("{card}-"))) {
            let base = PathBuf::from("/sys/class/drm").join(conn);
            let status = read_trim(&base.join("status")).unwrap_or_default();
            if status != "connected" {
                continue;
            }
            let vrr = read_trim(&base.join("vrr_capable")).is_some_and(|v| v == "1");
            out.push(f(
                Level::Info,
                "outputs",
                format!(
                    "{} connected, VRR {}",
                    conn.trim_start_matches(&format!("{card}-")),
                    if vrr { "capable" } else { "not reported" }
                ),
            ));
        }
    }
}

fn check_session(out: &mut Vec<Finding>) {
    let nested = std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    if nested {
        out.push(f(
            Level::Info,
            "session",
            "running inside another graphical session: hecate will start nested (winit)",
        ));
    }
    if std::env::var_os("XDG_SESSION_ID").is_some() {
        out.push(f(
            Level::Ok,
            "session",
            "logind session found (libseat will use it)",
        ));
    } else if Path::new("/run/seatd.sock").exists() {
        out.push(f(Level::Ok, "session", "seatd socket found"));
    } else if !nested {
        out.push(f(
            Level::Warn,
            "session",
            "no logind session or seatd socket: starting on a TTY may fail",
        ));
    }
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(d) if Path::new(&d).is_dir() => out.push(f(Level::Ok, "session", format!("XDG_RUNTIME_DIR={d}"))),
        _ => out.push(f(Level::Fail, "session", "XDG_RUNTIME_DIR is not set or missing")),
    }
}

fn check_tools(out: &mut Vec<Finding>, config: &crate::config::Config) {
    if config.xwayland.enable {
        if crate::utils::which(&config.xwayland.path) || Path::new(&config.xwayland.path).is_file() {
            out.push(f(
                Level::Ok,
                "xwayland",
                format!("{} found", config.xwayland.path),
            ));
        } else {
            out.push(f(
                Level::Warn,
                "xwayland",
                format!(
                    "{} not found: X11 apps (Steam, many games) won't start",
                    config.xwayland.path
                ),
            ));
        }
    }
    let portal_dirs = [
        "/run/current-system/sw/share/xdg-desktop-portal",
        "/usr/share/xdg-desktop-portal",
        "/etc/xdg-desktop-portal",
    ];
    let has_conf = portal_dirs.iter().any(|d| {
        Path::new(d).join("hecate-portals.conf").exists() || Path::new(d).join("portals.conf").exists()
    }) || directories::BaseDirs::new().is_some_and(|b| {
        b.config_dir()
            .join("xdg-desktop-portal")
            .join("hecate-portals.conf")
            .exists()
    });
    if has_conf {
        out.push(f(Level::Ok, "portals", "portal configuration found"));
    } else {
        out.push(f(
            Level::Warn,
            "portals",
            "no hecate-portals.conf: screen sharing may pick the wrong portal (the NixOS module installs one)",
        ));
    }
    for tool in ["xdg-desktop-portal-wlr", "dbus-update-activation-environment"] {
        let found = crate::utils::which(tool)
            || ["/run/current-system/sw/libexec", "/usr/libexec", "/usr/lib"]
                .iter()
                .any(|d| Path::new(d).join(tool).exists());
        out.push(f(
            if found { Level::Ok } else { Level::Warn },
            "tools",
            format!("{tool} {}", if found { "found" } else { "not found" }),
        ));
    }
}

pub fn run(config_path: Option<PathBuf>) -> Vec<Finding> {
    let mut out = Vec::new();
    let path = config_path
        .or_else(|| std::env::var_os("HECATE_CONFIG").map(PathBuf::from))
        .or_else(crate::config::load::default_path);
    let config = match &path {
        Some(p) if p.exists() => match crate::config::load_file(p) {
            Ok(c) => {
                out.push(f(Level::Ok, "config", format!("{} is valid", p.display())));
                c
            }
            Err(err) => {
                out.push(f(Level::Fail, "config", format!("{}: {err}", p.display())));
                crate::config::Config::default()
            }
        },
        Some(p) => {
            out.push(f(
                Level::Info,
                "config",
                format!("no config at {}, defaults apply", p.display()),
            ));
            crate::config::Config::default()
        }
        None => crate::config::Config::default(),
    };
    check_drm(&mut out);
    check_session(&mut out);
    check_tools(&mut out, &config);
    out
}

pub fn print(findings: &[Finding]) -> bool {
    let mut ok = true;
    for finding in findings {
        let tag = match finding.level {
            Level::Ok => "  ok ",
            Level::Info => "info ",
            Level::Warn => "warn ",
            Level::Fail => "FAIL ",
        };
        if finding.level == Level::Fail {
            ok = false;
        }
        println!("{tag} {:<9} {}", finding.topic, finding.message);
    }
    ok
}
