//! Protocol implementations not provided by Smithay.

pub mod ext_workspace;
pub mod foreign_toplevel;
pub mod gamma_control;
pub mod output_management;
pub mod output_power;

/// User data for resources that need none.
pub struct EmptyData;
