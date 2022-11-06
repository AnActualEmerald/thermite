use crate::api;
use crate::error::ThermiteError;
use crate::model;
use crate::model::LocalIndex;
use crate::model::ModVersion;
use crate::model::SubMod;
use directories::ProjectDirs;
use log::debug;
use log::error;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::path::PathBuf;

#[macro_export]
macro_rules! g2re {
    ($e:expr) => {{
        let re = $e.replace('*', ".*");
        regex::Regex::new(&re)
    }};
}

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        fs::create_dir_all(path.as_ref())?;
        Ok(TempDir {
            path: path.as_ref().to_path_buf(),
        })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.path) {
            error!(
                "Error removing temp directory at '{}': {}",
                self.path.display(),
                e
            );
        }
    }
}

///Takes the local and global installed files to display whether a mod is installed or not
pub async fn update_index<T: AsRef<Path>>(local: Option<T>, global: Option<T>) -> Vec<model::Mod> {
    let mut index = api::get_package_index().await.unwrap().to_vec();

    if let Some(local) = local {
        let installed = LocalIndex::load(local);
        for e in index.iter_mut() {
            if let Ok(installed) = &installed {
                for (name, i) in installed.mods.iter() {
                    if &e.name == name {
                        e.installed = true;
                        if e.versions.contains_key(&i.version) {
                            if let Some(v) = e.versions.get_mut(&i.version) {
                                v.installed = true;
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(global) = global {
        let glob = LocalIndex::load(global);
        for e in index.iter_mut() {
            if let Ok(glob) = &glob {
                for (name, i) in glob.mods.iter() {
                    if &e.name == name {
                        e.global = true;
                        if e.versions.contains_key(&i.version) {
                            if let Some(v) = e.versions.get_mut(&i.version) {
                                v.global = true;
                            }
                        }
                    }
                }
            }
        }
    }

    index
}

#[inline]
pub fn check_cache(path: &Path) -> Option<File> {
    if let Ok(f) = OpenOptions::new().read(true).open(path) {
        Some(f)
    } else {
        None
    }
}

#[inline(always)]
pub fn ensure_dirs(dirs: &ProjectDirs) {
    fs::create_dir_all(dirs.cache_dir()).unwrap();
    fs::create_dir_all(dirs.config_dir()).unwrap();
    fs::create_dir_all(dirs.data_local_dir()).unwrap();
}

pub fn remove_file(path: &Path) -> Result<(), ThermiteError> {
    fs::remove_file(path).map_err(|e| e.into())
}

//    pub fn remove_dir(dir: &Path) -> Result<(), String> {
//        fs::remove_dir_all(dir)
//            .map_err(|_| format!("Unable to remove directory {}", dir.display()))?;
//
//        Ok(())
//    }

pub fn clear_cache(dir: &Path, force: bool) -> Result<(), ThermiteError> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            clear_cache(&path, force)?;
            fs::remove_dir(&path)?;
        } else if path.extension() == Some(OsStr::new("zip")) || force {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

//    pub fn list_dir(dir: &Path) -> Result<Vec<String>, String> {
//        Ok(fs::read_dir(dir)
//            .map_err(|_| format!("unable to read directory {}", dir.display()))
//            .map_err(|_| format!("Unable to read directory {}", dir.display()))?
//            .filter(|f| f.is_ok())
//            .map(|f| f.unwrap())
//            .map(|f| f.file_name().to_string_lossy().into_owned())
//            .collect())
//    }

// #[inline]
// pub fn save_file(file: &Path, data: String) -> Result<()> {
//     fs::write(file, data.as_bytes())?;
//     Ok(())
// }

//    //supposing the mod name is formatted like Author.Mod@v1.0.0
//    pub fn parse_mod_name(name: &str) -> Option<String> {
//        let parts = name.split_once('.')?;
//        let author = parts.0;
//        //let parts = parts.1.split_once('@')?;
//        let m_name = parts.1;
//        //let ver = parts.1.replace('v', "");
//
//        let big_snake = Converter::new()
//            .set_delim("_")
//            .set_pattern(Pattern::Capital);
//
//        Some(format!("{}.{}", author, big_snake.convert(&m_name)))
//    }

///Returns a list of `Mod`s publled from an index based on the dep stings
///from Thunderstore
pub fn resolve_deps(
    deps: &[impl AsRef<str>],
    index: &[ModVersion],
) -> Result<Vec<ModVersion>, ThermiteError> {
    let mut valid = vec![];
    for dep in deps {
        let dep_name = dep
            .as_ref()
            .split('-')
            .nth(1)
            .ok_or_else(|| ThermiteError::DepError(dep.as_ref().into()))?;
        if let Some(d) = index.iter().find(|f| f.name == dep_name) {
            valid.push(d.clone());
        } else {
            return Err(ThermiteError::DepError(dep.as_ref().into()));
        }
    }
    Ok(valid)
}

pub fn disable_mod(dir: impl AsRef<Path>, m: &mut SubMod) -> Result<bool, ThermiteError> {
    if m.disabled() {
        return Ok(false);
    }

    let old_path = dir.as_ref().join(&m.path);

    let dir = dir.as_ref().join(".disabled");
    let new_path = dir.join(&m.path);

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    debug!(
        "Rename mod from {} to {}",
        old_path.display(),
        new_path.display()
    );
    fs::rename(&old_path, &new_path)?;

    m.path = Path::new(".disabled").join(&m.path);

    Ok(true)
}

pub fn enable_mod(dir: impl AsRef<Path>, m: &mut SubMod) -> Result<bool, ThermiteError> {
    if !m.disabled() {
        return Ok(false);
    }

    let old_path = dir.as_ref().join(&m.path);
    m.path = m.path.strip_prefix(".disabled").unwrap().to_path_buf();
    let new_path = dir.as_ref().join(&m.path);

    debug!(
        "Rename mod from {} to {}",
        old_path.display(),
        new_path.display()
    );

    fs::rename(old_path, new_path)?;

    Ok(true)
}
