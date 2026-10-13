//! Protocol implementations not provided by Smithay.

pub mod background_effect;
pub mod ext_workspace;
pub mod focus_grab;
pub mod foreign_toplevel;
pub mod gamma_control;
pub mod output_management;
pub mod output_power;
pub mod screencopy;
pub mod tearing_control;
pub mod toplevel_export;

/// User data for resources that need none.
pub struct EmptyData;
