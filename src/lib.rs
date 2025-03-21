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
//!         install_mod(&latest.full_name, Cursor::new(zipped), "packages").unwrap();
//!     }    
//! }
//! ```

pub mod api;
pub mod core;
pub mod error;
pub mod model;

/// The names of the Northstar core mods as found in their `mod.json` files, all lowercase
pub const CORE_MODS: [&str; 3] = [
    "northstar.custom",
    "northstar.customservers",
    "northstar.client",
];

/// Titanfall 2's Steam appid
pub const TITANFALL2_STEAM_ID: u32 = 1237970;
/// Titanfall 2's Origin/EA App ids
pub const TITANFALL2_ORIGIN_IDS: [&str; 2] = ["Origin.OFR.50.0001452", "Origin.OFR.50.0001456"];

// Important functions and structs
pub mod prelude {
    pub use crate::api::get_package_index;
    pub use crate::core::manage::{
        download, download_with_progress, install_mod, install_northstar, install_with_sanity,
    };

    pub use crate::core::utils::{find_mods, get_enabled_mods, resolve_deps};
    #[cfg(all(target_os = "linux", feature = "proton"))]
    pub use crate::core::{download_ns_proton, install_ns_proton, latest_release};
    #[cfg(feature = "steam")]
    pub use crate::core::{steam_dir, steam_libraries, titanfall2_dir};
    pub use crate::error::ThermiteError;
    pub use crate::CORE_MODS;
    pub use crate::TITANFALL2_STEAM_ID;
}
