//! # Basic Usage:
//! ```no_run
//! use thermite::prelude::*;
//! use std::io::Cursor;
//!
//! fn example() {
//!     let index = get_package_index().unwrap();
//!     if let Some(md) = index.iter().find(|e| e.name == "server_utilities") {
//!         let latest = md.get_latest().unwrap();
//!         let mut zipped = vec![];
//!         download(&mut zipped, &latest.url).unwrap();
//!         install_mod(&md.author, Cursor::new(zipped), "mods").unwrap();
//!     }    
//! }
//! ```

pub mod api;
pub mod core;
pub mod error;
pub mod model;

pub const CORE_MODS: [&str; 3] = [
    "northstar.custom",
    "northstar.customservers",
    "northstar.client",
];

// Important functions and structs
pub mod prelude {
    pub use crate::api::get_package_index;
    pub use crate::core::manage::{
        download, download_with_progress, install_mod, install_northstar, install_with_sanity,
        
    };

    #[deprecated]
    pub use crate::core::manage::uninstall;
    pub use crate::core::utils::{find_mods, get_enabled_mods, resolve_deps};
    #[cfg(all(target_os = "linux", feature = "proton"))]
    pub use crate::core::{download_ns_proton, install_ns_proton, latest_release};
    #[cfg(feature = "steam")]
    pub use crate::core::{steam_dir, steam_libraries, titanfall};
    pub use crate::error::ThermiteError;
    pub use crate::CORE_MODS;
}
