pub mod manage;
#[allow(dead_code)]
pub mod utils;

#[cfg(all(target_os = "linux", feature = "proton"))]
pub use utils::proton::{download_ns_proton, install_ns_proton, latest_release};
#[cfg(feature = "steam")]
pub use utils::steam::{steam_dir, steam_libraries, titanfall};
pub use utils::{find_mods, get_enabled_mods, resolve_deps};
