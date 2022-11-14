//! # Basic Usage:
//! ```no_run
//! use thermite::{Ctx, update_index, LocalIndex, ProjectDirs, install};
//! use std::path::Path;
//!
//! async fn example() {
//!     let index = update_index::<&Path>(None, None).await;
//!     let mut target = LocalIndex::load_or_create(Path::new("mods"));
//!     let mut ctx = Ctx::new(ProjectDirs::from("com", "YourOrg", "YourApp").unwrap());
//!     if let Some(md) = index.iter().find(|e| e.name == "server_utilities") {
//!         let latest = md.versions.get(&md.latest).unwrap();
//!         install(&mut ctx, &mut target, &[latest.clone()], false, true).await.unwrap();
//!     }    
//! }
//! ```

#[cfg(test)]
mod test;

pub mod api;
pub mod core;
pub mod error;
pub mod model;

// Re-exports
pub use directories::ProjectDirs;

// Important functions and structs
pub use crate::core::install_northstar;
pub use crate::core::utils::update_index;
pub use crate::core::{get_outdated, install, update, Ctx};
pub use crate::model::{LocalIndex, LocalMod, Mod, ModVersion};
