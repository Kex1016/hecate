//! Command line interface.

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

use crate::backend::{Backend, Headless, Tty, Winit};
use crate::state::State;

#[derive(Debug, Parser)]
#[command(name = "hecate", version = version(), about = "A stacking Wayland compositor")]
pub struct Cli {
    /// Config file (default: $XDG_CONFIG_HOME/hecate/config.toml)
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,
    /// Backend to use (default: winit when nested, udev otherwise)
    #[arg(long)]
    pub backend: Option<BackendKind>,
    /// Command to run once the compositor is up (e.g. a session script)
    #[arg(short = 's', long)]
    pub session_command: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BackendKind {
    Udev,
    Winit,
    Headless,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check a config file without starting the compositor
    Validate { path: Option<PathBuf> },
    /// Print the config JSON Schema
    Schema,
    /// Print the default config as TOML
    PrintDefaultConfig,
    /// Diagnose the environment: GPU, outputs, session, tools, config
    Check,
    /// Talk to the running compositor (windows, workspaces, actions, pick, …)
    Msg(crate::ipc::client::MsgArgs),
    /// Generate shell completions
    Completions { shell: clap_complete::Shell },
}

fn version() -> String {
    let rev = option_env!("HECATE_GIT_REV").unwrap_or("dev");
    format!("{} ({rev})", env!("CARGO_PKG_VERSION"))
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Some(Command::Validate { path }) => validate(path.or(cli.config)),
        Some(Command::Schema) => {
            let schema = schemars::schema_for!(crate::config::Config);
            println!("{}", serde_json::to_string_pretty(&schema)?);
            Ok(())
        }
        Some(Command::PrintDefaultConfig) => {
            let mut config = crate::config::Config::default();
            config.binds = crate::config::defaults::base_binds().clone();
            config.mouse_binds = crate::config::defaults::base_mouse_binds().clone();
            print!("{}", toml::to_string_pretty(&config)?);
            Ok(())
        }
        Some(Command::Msg(args)) => crate::ipc::client::run(args),
        Some(Command::Check) => {
            let findings = crate::check::run(cli.config);
            if !crate::check::print(&findings) {
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::Completions { shell }) => {
            clap_complete::generate(shell, &mut Cli::command(), "hecate", &mut std::io::stdout());
            Ok(())
        }
        None => run_compositor(cli.config, cli.backend, cli.session_command),
    }
}

fn validate(path: Option<PathBuf>) -> anyhow::Result<()> {
    let path = path.or_else(crate::config::load::default_path);
    let Some(path) = path else {
        anyhow::bail!("no config path given and no default location");
    };
    match crate::config::load_file(&path) {
        Ok(_) => {
            println!("{}: ok", path.display());
            Ok(())
        }
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            std::process::exit(1);
        }
    }
}

fn init_logging() {
    let filter = std::env::var("HECATE_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "hecate=info,smithay=warn".into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .try_init();
}

fn run_compositor(
    config_path: Option<PathBuf>,
    backend: Option<BackendKind>,
    session_command: Option<String>,
) -> anyhow::Result<()> {
    init_logging();
    let (config, config_path) = match crate::config::load(config_path.as_deref()) {
        Ok(c) => c,
        Err(err) => {
            tracing::error!("config error, starting with defaults: {err}");
            (crate::config::Config::default(), config_path)
        }
    };

    let mut event_loop: EventLoop<State> = EventLoop::try_new()?;
    let display: Display<State> = Display::new()?;
    let nested = std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some();
    let kind = backend.unwrap_or(if nested {
        BackendKind::Winit
    } else {
        BackendKind::Udev
    });
    let backend = match kind {
        BackendKind::Winit => Backend::Winit(Winit::new(event_loop.handle())?),
        BackendKind::Udev => Backend::Tty(Tty::new(event_loop.handle())?),
        BackendKind::Headless => Backend::Headless(Headless::new()),
    };

    let mut state = State::new(
        config,
        config_path,
        event_loop.handle(),
        event_loop.get_signal(),
        display,
        backend,
    )?;
    if let State {
        backend: Backend::Headless(h),
        hecate,
    } = &mut state
    {
        h.add_output(hecate, "headless-1", (1920, 1080));
    }

    if let Err(err) = crate::ipc::IpcServer::start(&mut state) {
        tracing::warn!("IPC disabled: {err}");
    }
    let socket = state.hecate.socket_name.clone().unwrap_or_default();
    tracing::info!("listening on WAYLAND_DISPLAY={socket}");
    // SAFETY: set before spawning any threads that read the environment.
    unsafe {
        std::env::set_var("WAYLAND_DISPLAY", &socket);
        std::env::set_var("XDG_CURRENT_DESKTOP", "hecate");
        std::env::set_var("XDG_SESSION_TYPE", "wayland");
        std::env::remove_var("DISPLAY");
    }
    crate::utils::set_child_env("WAYLAND_DISPLAY", Some(&socket));
    crate::utils::set_child_env("XDG_CURRENT_DESKTOP", Some("hecate"));
    crate::utils::set_child_env("XDG_SESSION_TYPE", Some("wayland"));
    for (k, v) in state.hecate.config.environment.clone() {
        crate::utils::set_child_env(&k, Some(&v));
    }

    if let Some(path) = state.hecate.config_path.clone() {
        match crate::config::watch::watch(&path, &state.hecate.event_loop) {
            Ok(w) => state.hecate.config_watcher = Some(w),
            Err(err) => tracing::warn!("not watching the config for changes: {err}"),
        }
    }
    // SIGHUP reloads the config.
    if let Ok(signals) = smithay::reexports::calloop::signals::Signals::new(&[
        smithay::reexports::calloop::signals::Signal::SIGHUP,
    ]) {
        let _ = state
            .hecate
            .event_loop
            .insert_source(signals, |_, _, state| state.reload_config());
    }

    state.start_session();
    for argv in state.hecate.config.autostart.clone() {
        state.hecate.spawn(&argv);
    }
    if let Some(cmd) = session_command {
        state.hecate.spawn(&["sh".into(), "-c".into(), cmd]);
    }

    event_loop.run(None, &mut state, |state| state.refresh_and_flush_clients())?;
    state.end_session();
    Ok(())
}
