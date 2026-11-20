//! Talking to the compositor: the same socket and requests as `hecate msg`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use hecate_ipc::{self as ipc, Request};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Longest a request may take. Nothing the server sends waits for the user.
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Hecate {
    /// Fixed socket (tests, `--socket`); otherwise found on each request, so the server
    /// survives a compositor restart.
    socket: Option<PathBuf>,
}

impl Hecate {
    pub fn new(socket: Option<PathBuf>) -> Hecate {
        Hecate { socket }
    }

    /// `$HECATE_SOCKET`, else the newest live `hecate.*.sock` in `$XDG_RUNTIME_DIR`
    /// (preferring the one for `$WAYLAND_DISPLAY`): Claude Code may have been started from
    /// somewhere that does not pass the session's environment on.
    pub fn socket_path(&self) -> Result<PathBuf, String> {
        if let Some(p) = &self.socket {
            return Ok(p.clone());
        }
        if let Some(p) = std::env::var_os(ipc::SOCKET_ENV).filter(|p| !p.is_empty()) {
            return Ok(PathBuf::from(p));
        }
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .ok_or_else(|| format!("neither {} nor XDG_RUNTIME_DIR is set", ipc::SOCKET_ENV))?;
        let display = std::env::var("WAYLAND_DISPLAY").ok();
        find_socket(Path::new(&runtime), display.as_deref())
            .ok_or_else(|| "no running hecate found (no live hecate.*.sock in XDG_RUNTIME_DIR)".into())
    }

    /// The Wayland display the compositor serves, for grim: `$WAYLAND_DISPLAY`, else the
    /// one in the socket's name.
    pub fn wayland_display(&self) -> Option<String> {
        if let Ok(d) = std::env::var("WAYLAND_DISPLAY")
            && !d.is_empty()
        {
            return Some(d);
        }
        let path = self.socket_path().ok()?;
        display_from_socket_name(path.file_name()?.to_str()?)
    }

    /// Send a request and return its `ok` value.
    pub fn request_value(&self, req: &Request) -> Result<Value, String> {
        let path = self.socket_path()?;
        let mut stream = UnixStream::connect(&path)
            .map_err(|e| format!("connecting to hecate at {}: {e}", path.display()))?;
        let _ = stream.set_read_timeout(Some(TIMEOUT));
        let _ = stream.set_write_timeout(Some(TIMEOUT));
        let mut line = serde_json::to_vec(req).map_err(|e| e.to_string())?;
        line.push(b'\n');
        stream
            .write_all(&line)
            .map_err(|e| format!("writing to hecate: {e}"))?;
        let mut reply = String::new();
        BufReader::new(stream)
            .read_line(&mut reply)
            .map_err(|e| format!("reading from hecate: {e}"))?;
        let v: Value = serde_json::from_str(&reply).map_err(|e| format!("bad reply from hecate: {e}"))?;
        if let Some(err) = v.get("error") {
            return Err(err.as_str().unwrap_or("error").to_string());
        }
        Ok(v.get("ok").cloned().unwrap_or(Value::Null))
    }

    /// Send a request and read its reply as `T`.
    pub fn request<T: DeserializeOwned>(&self, req: &Request) -> Result<T, String> {
        let v = self.request_value(req)?;
        serde_json::from_value(v).map_err(|e| format!("unexpected reply from hecate: {e}"))
    }

    /// Run an action, on a window when `target` is set.
    pub fn action(&self, action: Value, target: Option<ipc::Selector>) -> Result<(), String> {
        self.request_value(&Request::Action { action, target })
            .map(|_| ())
    }
}

/// `hecate.wayland-1.1234.sock` → `wayland-1`.
fn display_from_socket_name(name: &str) -> Option<String> {
    let rest = name.strip_prefix("hecate.")?.strip_suffix(".sock")?;
    let (display, pid) = rest.rsplit_once('.')?;
    pid.parse::<u32>().ok()?;
    Some(display.to_string())
}

fn find_socket(runtime: &Path, display: Option<&str>) -> Option<PathBuf> {
    let mut found: Vec<(bool, std::time::SystemTime, PathBuf)> = std::fs::read_dir(runtime)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let shown = display_from_socket_name(&name)?;
            let modified = e.metadata().and_then(|m| m.modified()).ok()?;
            Some((Some(shown.as_str()) == display, modified, e.path()))
        })
        .collect();
    // Matching display first, then newest.
    found.sort_by_key(|a| std::cmp::Reverse((a.0, a.1)));
    found
        .into_iter()
        .map(|(_, _, p)| p)
        .find(|p| UnixStream::connect(p).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_in_socket_names() {
        assert_eq!(
            display_from_socket_name("hecate.wayland-1.42.sock").as_deref(),
            Some("wayland-1")
        );
        assert_eq!(
            display_from_socket_name("hecate.wayland.x.7.sock").as_deref(),
            Some("wayland.x")
        );
        assert_eq!(display_from_socket_name("hecate.wayland-1.sock"), None);
        assert_eq!(display_from_socket_name("other.sock"), None);
    }

    #[test]
    fn finds_a_live_socket_and_skips_stale_ones() {
        let dir = std::env::temp_dir().join(format!("hecate-mcp-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A stale one: a socket file nobody listens on.
        drop(std::os::unix::net::UnixListener::bind(dir.join("hecate.wayland-9.1.sock")).unwrap());
        assert_eq!(find_socket(&dir, Some("wayland-9")), None);
        let _live = std::os::unix::net::UnixListener::bind(dir.join("hecate.wayland-1.2.sock")).unwrap();
        assert_eq!(
            find_socket(&dir, Some("wayland-9")),
            Some(dir.join("hecate.wayland-1.2.sock"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
