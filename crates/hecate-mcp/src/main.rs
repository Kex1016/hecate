//! hecate-mcp: a Model Context Protocol server (stdio) for the hecate compositor.
//!
//! It asks the compositor what `hecate msg` would, over the same socket: windows,
//! workspaces, outputs; reads the shell's notification history; and takes screenshots with
//! grim. It only looks unless told otherwise: focusing and moving windows, switching
//! workspaces, border tints and sending notifications each need `--allow` or `allow` in
//! `~/.config/hecate/mcp.toml`.
//!
//! Claude Code: `claude mcp add hecate -- hecate-mcp` (add `--allow tint` and so on after it).

mod config;
mod hecate;
mod mcp;
mod tools;

use std::path::PathBuf;

use clap::Parser;

use crate::config::{Config, Permission};

#[derive(Debug, Parser)]
#[command(
    name = "hecate-mcp",
    version,
    about = "MCP server (stdio) for the hecate compositor"
)]
struct Args {
    /// Let it change things: windows (focus, move, switch workspace), tint, notify, or all.
    /// Comma-separated or repeated. Adds to `allow` in the config file
    #[arg(long, value_delimiter = ',', value_name = "WHAT")]
    allow: Vec<String>,
    /// Change nothing, whatever the config file allows
    #[arg(long, conflicts_with = "allow")]
    read_only: bool,
    /// Leave the screenshot tool out
    #[arg(long)]
    no_screenshots: bool,
    /// Config file [default: $XDG_CONFIG_HOME/hecate/mcp.toml]
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    /// Compositor socket [default: $HECATE_SOCKET, else the live one in $XDG_RUNTIME_DIR]
    #[arg(long, value_name = "PATH")]
    socket: Option<PathBuf>,
    /// Print the tools it would offer, as JSON, and exit
    #[arg(long)]
    list_tools: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let config = load_config(&args)?;
    let tools = tools::Tools::new(config, hecate::Hecate::new(args.socket.clone()));
    if args.list_tools {
        println!("{}", serde_json::to_string_pretty(&tools.list())?);
        return Ok(());
    }
    // stdout carries the protocol; anything for people goes to stderr.
    eprintln!(
        "hecate-mcp {}: {}",
        env!("CARGO_PKG_VERSION"),
        tools.instructions()
    );
    let stdin = std::io::stdin().lock();
    let stdout = std::io::stdout().lock();
    mcp::Server::new(tools).run(stdin, stdout)?;
    Ok(())
}

fn load_config(args: &Args) -> anyhow::Result<Config> {
    let mut config = Config::default();
    let path = args.config.clone().or_else(config::default_config_file);
    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(text) => config
                .apply_toml(&text)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?,
            // Only a file named on the command line has to exist.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && args.config.is_none() => {}
            Err(e) => anyhow::bail!("{}: {e}", path.display()),
        }
    }
    for name in &args.allow {
        config
            .allow
            .extend(Permission::parse(name).map_err(anyhow::Error::msg)?);
    }
    if args.read_only {
        config.allow.clear();
    }
    if args.no_screenshots {
        config.screenshots = false;
    }
    Ok(config)
}
