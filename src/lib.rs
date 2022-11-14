//! # Basic Usage:
//! ```no_run
//! use thermite::api::get_package_index;
//! use thermite::download_file;
//! use thermite::install_mod;
//! use std::path::Path;
//!
//! async fn example() {
//!     let index = get_package_index().await.unwrap();
//!     if let Some(md) = index.iter().find(|e| e.name == "server_utilities") {
//!         let latest = md.get_latest().unwrap();
//!         let zipped = download_file(&latest.url, Path::new("server_utils.zip")).await.unwrap();
//!         install_mod(&zipped, Path::new("mods")).unwrap();
//!     }    
//! }
//! ```

#[cfg(test)]
mod test;

pub mod api;
pub mod core;
pub mod error;
pub mod model;

// Important functions and structs
pub use crate::core::manage::*;
pub use crate::core::utils::{find_mods, get_enabled_mods, resolve_deps};
