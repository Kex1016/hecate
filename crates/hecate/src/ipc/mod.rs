//! IPC: control socket server and payload builders. Types live in the `hecate-ipc` crate.

pub mod client;
pub mod payload;
pub mod server;

pub use server::IpcServer;
