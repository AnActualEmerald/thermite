#[cfg(test)]
mod test {}

pub mod api;
pub mod core;
pub mod error;
pub mod model;

// Re-exports
pub use directories::ProjectDirs;

// Important functions and structs
pub use crate::core::utils::update_index;
pub use crate::core::{get_outdated, install, update};
pub use crate::model::{LocalIndex, LocalMod, Mod};
