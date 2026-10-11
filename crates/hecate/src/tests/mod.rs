//! Integration tests: the compositor runs headless in-process and real Wayland clients talk
//! to it over a socket pair. Render tests use a surfaceless EGL renderer (Mesa llvmpipe in the
//! Nix devShell) and check pixels.

mod client;
mod decorations;
mod ext_workspace;
mod fixture;
mod focus_grab;
mod foreign_toplevel;
mod gamma;
mod image_copy_capture;
mod ipc;
mod layers;
mod output_management;
mod output_power;
mod render;
mod screencopy;
mod windows;
mod workspaces;
