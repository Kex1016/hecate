//! hecate: a stacking Wayland compositor.

mod actions;
mod backend;
mod cli;
mod config;
mod cursor;
mod handlers;
mod input;
mod render;
mod session;
mod state;
mod utils;
mod wm;

#[cfg(test)]
mod tests;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli::run(cli)
}
