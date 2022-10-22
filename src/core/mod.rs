use crate::model::Cache;
use directories::ProjectDirs;

pub mod actions;
mod install;
mod northstar;
mod update;
#[allow(dead_code)]
pub mod utils;

pub use install::install;
pub use northstar::install_northstar;
pub use update::{get_outdated, update};
pub use utils::{resolve_deps, update_index};

/// Tracks context info including the package cache
#[derive(Debug, Clone)]
pub struct Ctx {
    pub cache: Cache,
    pub dirs: ProjectDirs,
}

impl Ctx {
    pub fn new(dirs: ProjectDirs) -> Self {
        utils::ensure_dirs(&dirs);
        let cache = Cache::build(dirs.cache_dir()).unwrap();
        Ctx { dirs, cache }
    }

    pub fn no_cache(dirs: ProjectDirs) -> Self {
        utils::ensure_dirs(&dirs);
        Ctx {
            dirs,
            cache: Cache::default(),
        }
    }
}
