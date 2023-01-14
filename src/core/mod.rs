pub mod manage;
#[allow(dead_code)]
pub mod utils;

#[cfg(feature = "steam")]
pub use utils::steam::{steam_libraries, titanfall};
pub use utils::{find_mods, get_enabled_mods, resolve_deps};
