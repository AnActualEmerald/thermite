use crate::error::ThermiteError;
use crate::model::EnabledMods;
use crate::model::InstalledMod;
use crate::model::Mod;

use log::debug;
use log::error;
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;

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

impl Deref for TempDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
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

///Returns a list of `Mod`s publled from an index based on the dep stings
///from Thunderstore
pub fn resolve_deps(deps: &[impl AsRef<str>], index: &[Mod]) -> Result<Vec<Mod>, ThermiteError> {
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

/// Get `enabledmods.json` from the given directory, if it exists
pub fn get_enabled_mods(dir: impl AsRef<Path>) -> Result<EnabledMods, ThermiteError> {
    let path = dir.as_ref().join("enabledmods.json");
    if path.exists() {
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    } else {
        Err(ThermiteError::MissingFile(dir.as_ref().to_path_buf()))
    }
}

/// Search a directory for mod.json files in its children
///
/// Searches one level deep
pub fn find_mods(
    dir: impl AsRef<Path>,
) -> Result<Vec<Result<InstalledMod, ThermiteError>>, ThermiteError> {
    let mut res = vec![];
    let dir = dir.as_ref().canonicalize()?;
    debug!("Finding mods in '{}'", dir.display());
    for child in dir.read_dir()? {
        let child = child?;
        if !child.file_type()?.is_dir() {
            debug!("Skipping file {}", child.path().display());
            continue;
        }
        let path = child.path().join("mod.json");
        let mod_json = if path.try_exists()? {
            let raw = fs::read_to_string(&path)?;
            let Ok(parsed) = json5::from_str(&raw) else {
                res.push(Err(ThermiteError::MiscError(format!("Error parsing {}", path.display()))));
                continue;
            };
            parsed
        } else {
            continue;
        };
        let path = child.path().join("manifest.json");
        let manifest = if path.try_exists()? {
            let raw = fs::read_to_string(&path)?;
            let Ok(parsed) = serde_json::from_str(&raw) else {
                res.push(Err(ThermiteError::MiscError(format!("Error parsing {}", path.display()))));
                continue;
            };
            parsed
        } else {
            continue;
        };
        let path = child.path().join("thunderstore_author.txt");
        let author = if path.try_exists()? {
            fs::read_to_string(path)?
        } else {
            continue;
        };

        res.push(Ok(InstalledMod {
            manifest,
            mod_json,
            author,
            path: child.path(),
        }));
    }

    Ok(res)
}

#[cfg(feature = "steam")]
pub(crate) mod steam {
    use std::path::PathBuf;
    use steamlocate::SteamDir;

    pub fn steam_libraries() -> Option<Vec<PathBuf>> {
        let mut steamdir = SteamDir::locate()?;
        let folders = steamdir.libraryfolders();
        Some(folders.paths.clone())
    }

    pub fn titanfall() -> Option<PathBuf> {
        let mut steamdir = SteamDir::locate()?;
        Some(steamdir.app(&1237970)?.path.clone())
    }
}
