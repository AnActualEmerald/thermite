use crate::error::ThermiteError;
use crate::model::EnabledMods;
use crate::model::InstalledMod;
use crate::model::Mod;

use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use tracing::{debug, error};

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

        if dep_name.to_lowercase() == "northstar" {
            debug!("Skip unfiltered Northstar dependency");
            continue;
        }

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
    let path = dir.as_ref().canonicalize()?.join("enabledmods.json");
    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let mut mods: EnabledMods = serde_json::from_str(&raw)?;
        mods.set_path(path);
        Ok(mods)
    } else {
        Err(ThermiteError::MissingFile(Box::new(path)))
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

    pub fn steam_dir() -> Option<PathBuf> {
        SteamDir::locate().map(|v| v.path)
    }

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

#[cfg(all(target_os = "linux", feature = "proton"))]
pub(crate) mod proton {
    use flate2::read::GzDecoder;
    use std::{fs::File, io::Write, path::Path};
    use tar::Archive;
    use tracing::debug;

    use crate::{
        core::manage::download,
        error::{Result, ThermiteError},
    };
    const BASE_URL: &str = "https://github.com/cyrv6737/NorthstarProton/releases/";

    /// Returns the latest tag from the NorthstarProton repo
    pub fn latest_release() -> Result<String> {
        let url = format!("{}latest", BASE_URL);
        let res = ureq::get(&url).call()?;
        debug!("{:#?}", res);
        let location = res.get_url();

        Ok(location
            .split('/')
            .last()
            .ok_or_else(|| ThermiteError::MiscError("Malformed location URL".into()))?
            .to_owned())
    }
    /// Convinience function for downloading a given tag from the NorthstarProton repo
    pub fn download_ns_proton(tag: impl AsRef<str>, output: impl Write) -> Result<u64> {
        let url = format!(
            "{}download/{}/NorthstarProton-{}.tar.gz",
            BASE_URL,
            tag.as_ref(),
            tag.as_ref().trim_matches('v')
        );
        download(output, url)
    }

    /// Extract the NorthstarProton tarball into a given directory
    pub fn install_ns_proton(archive: &File, dest: impl AsRef<Path>) -> Result<()> {
        let mut tarball = Archive::new(GzDecoder::new(archive));
        tarball.unpack(dest)?;

        Ok(())
    }
}
