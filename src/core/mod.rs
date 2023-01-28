pub mod manage;
#[allow(dead_code)]
pub mod utils;

#[cfg(feature = "steam")]
pub use utils::steam::{steam_libraries, titanfall};
#[cfg(all(target_os = "linux", feature = "proton"))]
pub use utils::proton::{latest_release, download_ns_proton, install_ns_proton};
pub use utils::{find_mods, get_enabled_mods, resolve_deps};
