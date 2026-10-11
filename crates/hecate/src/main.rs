//! hecate: a stacking Wayland compositor.

// Wayland resources and outputs hash by identity; their interior mutability doesn't touch the key.
#![allow(clippy::mutable_key_type)]

mod actions;
mod backend;
mod check;
mod cli;
mod config;
mod cursor;
mod handlers;
mod input;
mod ipc;
mod overrides;
mod protocols;
mod render;
mod session;
mod state;
mod tools;
mod utils;
mod wm;

#[cfg(test)]
mod tests;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli::run(cli)
}
