//! X11 through xwayland-satellite, started on the first X11 connection.
//!
//! hecate owns the X11 display sockets and hands them to satellite with `-listenfd` when a
//! client connects, so X11 costs nothing until something uses it. If satellite exits (crash,
//! or idle-exit), the sockets are watched again and the next client starts it anew.
//!
//! Adapted from niri (src/utils/xwayland/), GPL-3.0-or-later; socket setup follows Mutter.

use std::fs::{DirBuilder, OpenOptions};
use std::io::Write as _;
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::linux::net::SocketAddrExt as _;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::net::{SocketAddr, UnixListener};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context as _, anyhow, ensure};
use smithay::reexports::calloop::channel::{self, Sender};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, Mode, PostAction, RegistrationToken};

use crate::state::State;

/// Removes a file when dropped.
struct Unlink(PathBuf);

impl Drop for Unlink {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct X11Connection {
    display_name: String,
    abstract_fd: OwnedFd,
    unix_fd: OwnedFd,
    _unix_guard: Unlink,
    _lock_guard: Unlink,
}

pub struct Satellite {
    x11: X11Connection,
    path: String,
    /// Socket watches, tagged by index (0 abstract, 1 unix).
    tokens: Vec<(usize, RegistrationToken)>,
    to_main: Sender<()>,
    /// Times satellite was started (tests, `hecate check`).
    pub spawn_count: u32,
}

impl Satellite {
    pub fn display_name(&self) -> &str {
        &self.x11.display_name
    }
}

fn ensure_x11_unix_dir(tmp: &Path) -> anyhow::Result<PathBuf> {
    let dir = tmp.join(".X11-unix");
    match DirBuilder::new().mode(0o1777).create(&dir) {
        Ok(()) => {
            // The umask applies to mkdir.
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o1777))
                .context("setting X11 directory permissions")?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let meta = std::fs::symlink_metadata(&dir).context("checking the X11 directory")?;
            let tmp_meta = std::fs::symlink_metadata(tmp).context("checking the tmp directory")?;
            // SAFETY: getuid has no failure modes.
            let uid = unsafe { libc::getuid() };
            ensure!(meta.is_dir(), "{} is not a directory", dir.display());
            ensure!(
                meta.uid() == tmp_meta.uid() || meta.uid() == uid,
                "wrong ownership for {}",
                dir.display()
            );
            ensure!(meta.mode() & 0o022 == 0o022, "{} is not writable", dir.display());
            ensure!(
                meta.mode() & 0o1000 != 0,
                "{} is missing the sticky bit",
                dir.display()
            );
        }
        Err(err) => return Err(err).context("creating the X11 directory"),
    }
    Ok(dir)
}

fn setup_connection(tmp: &Path) -> anyhow::Result<X11Connection> {
    let x11_dir = ensure_x11_unix_dir(tmp)?;
    for n in 0..64u32 {
        let lock_path = tmp.join(format!(".X{n}-lock"));
        let Ok(mut lock) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o444)
            .open(&lock_path)
        else {
            continue;
        };
        let lock_guard = Unlink(lock_path);
        lock.write_all(format!("{:>10}\n", std::process::id()).as_bytes())
            .context("writing the X11 lock file")?;
        drop(lock);

        let socket = x11_dir.join(format!("X{n}"));
        let abstract_addr = SocketAddr::from_abstract_name(socket.as_os_str().as_encoded_bytes())?;
        let Ok(abstract_listener) = UnixListener::bind_addr(&abstract_addr) else {
            continue;
        };
        // A stale socket from a dead server can be replaced since we hold the lock.
        let _ = std::fs::remove_file(&socket);
        let Ok(unix_listener) = UnixListener::bind(&socket) else {
            continue;
        };
        return Ok(X11Connection {
            display_name: format!(":{n}"),
            abstract_fd: abstract_listener.into(),
            unix_fd: unix_listener.into(),
            _unix_guard: Unlink(socket),
            _lock_guard: lock_guard,
        });
    }
    Err(anyhow!("no free X11 display"))
}

/// Log xwayland-satellite's output line by line, under hecate's own target so the default
/// `hecate=info` filter shows it.
fn forward_output(stream: Option<impl std::io::Read + Send + 'static>) {
    let Some(stream) = stream else {
        return;
    };
    let res = std::thread::Builder::new()
        .name("satellite-log".into())
        .spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                tracing::info!(target: "hecate::xwayland_satellite", "{line}");
            }
        });
    if let Err(err) = res {
        tracing::warn!("error starting the satellite log thread: {err}");
    }
}

/// Whether `path` is an xwayland-satellite that can take listening sockets.
fn supports_listenfd(path: &str) -> bool {
    let status = Command::new(crate::utils::expand_home(path))
        .args([":0", "--test-listenfd-support"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("DISPLAY")
        .status();
    match status {
        Ok(s) if s.success() => true,
        Ok(_) => {
            tracing::warn!("{path} doesn't support on-demand activation (-listenfd); X11 disabled");
            false
        }
        Err(err) => {
            tracing::warn!("can't run {path} ({err}); X11 disabled");
            false
        }
    }
}

impl State {
    /// Open the X11 sockets and wait for clients. Sets `DISPLAY` for children.
    pub fn setup_xwayland(&mut self) {
        self.setup_xwayland_in(Path::new("/tmp"));
    }

    pub fn setup_xwayland_in(&mut self, tmp: &Path) {
        if self.hecate.satellite.is_some() || !self.hecate.config.xwayland.enable {
            return;
        }
        let path = self.hecate.config.xwayland.path.clone();
        if !supports_listenfd(&path) {
            return;
        }
        let x11 = match setup_connection(tmp) {
            Ok(x) => x,
            Err(err) => {
                tracing::warn!("error opening X11 sockets, X11 disabled: {err:?}");
                return;
            }
        };
        let (to_main, rx) = channel::channel();
        let res = self.hecate.event_loop.insert_source(rx, |event, _, state| {
            if let channel::Event::Msg(()) = event {
                state.watch_x11_sockets();
            }
        });
        if let Err(err) = res {
            tracing::warn!("error setting up xwayland-satellite: {err}");
            return;
        }
        tracing::info!("X11 display {} (xwayland-satellite on demand)", x11.display_name);
        crate::utils::set_child_env("DISPLAY", Some(&x11.display_name));
        self.hecate.satellite = Some(Satellite {
            x11,
            path,
            tokens: Vec::new(),
            to_main,
            spawn_count: 0,
        });
        self.watch_x11_sockets();
    }

    /// Close the X11 sockets (xwayland disabled at runtime). A running satellite keeps going
    /// until its clients leave.
    pub fn teardown_xwayland(&mut self) {
        if let Some(s) = self.hecate.satellite.take() {
            for (_, token) in s.tokens {
                self.hecate.event_loop.remove(token);
            }
            crate::utils::set_child_env("DISPLAY", None);
        }
    }

    fn watch_x11_sockets(&mut self) {
        let Some(s) = self.hecate.satellite.as_mut() else {
            return;
        };
        for (_, token) in s.tokens.drain(..) {
            self.hecate.event_loop.remove(token);
        }
        for (idx, fd) in [&s.x11.abstract_fd, &s.x11.unix_fd].into_iter().enumerate() {
            let Ok(fd) = fd.try_clone() else {
                continue;
            };
            // If satellite failed before accepting, pending connections would keep the source
            // firing; drop them (their clients would fail anyway).
            let fd = clear_pending(fd);
            let source = Generic::new(fd, Interest::READ, Mode::Level);
            let token = self.hecate.event_loop.insert_source(source, move |_, _, state| {
                state.spawn_satellite(idx);
                // This source removes itself; spawn_satellite removed the other one.
                Ok(PostAction::Remove)
            });
            match token {
                Ok(t) => s.tokens.push((idx, t)),
                Err(err) => tracing::warn!("error watching X11 socket: {err}"),
            }
        }
    }

    fn spawn_satellite(&mut self, firing: usize) {
        let Some(s) = self.hecate.satellite.as_mut() else {
            return;
        };
        // Satellite owns the sockets until it exits.
        for (idx, token) in s.tokens.drain(..) {
            if idx != firing {
                self.hecate.event_loop.remove(token);
            }
        }
        s.spawn_count += 1;
        tracing::debug!("X11 client connected; starting xwayland-satellite");
        let fds = (s.x11.abstract_fd.try_clone(), s.x11.unix_fd.try_clone());
        let (Ok(abstract_fd), Ok(unix_fd)) = fds else {
            let _ = s.to_main.send(());
            return;
        };
        let mut command = Command::new(crate::utils::expand_home(&s.path));
        command
            .arg(&s.x11.display_name)
            .arg("-listenfd")
            .arg(unix_fd.as_raw_fd().to_string())
            .arg("-listenfd")
            .arg(abstract_fd.as_raw_fd().to_string())
            .env_remove("DISPLAY")
            .env_remove("RUST_BACKTRACE")
            .stdin(Stdio::null())
            // Its messages (which X11 windows it maps, why it drops one) go into hecate's log.
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in crate::utils::child_env() {
            if k == "DISPLAY" {
                continue;
            }
            match v {
                Some(v) => command.env(k, v),
                None => command.env_remove(k),
            };
        }
        let raws = [unix_fd.as_raw_fd(), abstract_fd.as_raw_fd()];
        // SAFETY: only async-signal-safe calls between fork and exec.
        unsafe {
            command.pre_exec(move || {
                for fd in raws {
                    if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                crate::utils::unblock_signals();
                Ok(())
            });
        }
        let to_main = s.to_main.clone();
        let res = std::thread::Builder::new()
            .name("satellite".into())
            .spawn(move || {
                match command.spawn() {
                    Ok(mut child) => {
                        drop((abstract_fd, unix_fd));
                        forward_output(child.stdout.take());
                        forward_output(child.stderr.take());
                        match child.wait() {
                            Ok(status) => tracing::info!("xwayland-satellite exited: {status}"),
                            Err(err) => tracing::warn!("error waiting for xwayland-satellite: {err}"),
                        }
                    }
                    Err(err) => tracing::warn!("error starting xwayland-satellite: {err}"),
                }
                // Watch again so the next X11 client restarts it.
                let _ = to_main.send(());
            });
        if let Err(err) = res {
            tracing::warn!("error starting satellite thread: {err}");
            if let Some(s) = &self.hecate.satellite {
                let _ = s.to_main.send(());
            }
        }
    }
}

fn clear_pending(fd: OwnedFd) -> OwnedFd {
    let listener = UnixListener::from(fd);
    if listener.set_nonblocking(true).is_ok() {
        while listener.accept().is_ok() {}
        let _ = listener.set_nonblocking(false);
    }
    listener.into()
}
