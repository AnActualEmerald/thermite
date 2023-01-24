//! # Basic Usage:
//! ```no_run
//! use thermite::prelude::*;
//!
//! fn example() {
//!     let index = get_package_index().unwrap();
//!     if let Some(md) = index.iter().find(|e| e.name == "server_utilities") {
//!         let latest = md.get_latest().unwrap();
//!         let zipped = download_file(&latest.url, "server_utils.zip").unwrap();
//!         install_mod(&md.author, &zipped, "mods").unwrap();
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
pub mod prelude {
    pub use crate::api::get_package_index;
    pub use crate::core::manage::{
        download_file, download_file_with_progress, install_mod, install_northstar,
        install_with_sanity, uninstall,
    };
    pub use crate::core::utils::{find_mods, get_enabled_mods, resolve_deps};
    #[cfg(feature = "steam")]
    pub use crate::core::{steam_libraries, titanfall};
    pub use crate::error::ThermiteError;
}
