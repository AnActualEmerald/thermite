use crate::error::ThermiteError;
use crate::model::EnabledMods;
use crate::model::InstalledMod;
use crate::model::Manifest;
use crate::model::Mod;

use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::trace;
use tracing::{debug, error};

pub(crate) type ModString = (String, String, Option<String>);

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    /// # Errors
    /// - IO errors
    pub fn create(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        fs::create_dir_all(path.as_ref())?;
        Ok(TempDir {
            path: path.as_ref().to_path_buf(),
        })
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.path
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

// pub(crate) struct StaticRef<T> {
//     cell: OnceLock<T>,
//     init: std::sync::Arc<dyn Fn() -> T>,
// }

// impl<T> StaticRef<T> {
//     pub const fn new(f: dyn Fn() -> T) -> Self {
//         Self {
//             cell: OnceLock::new(),
//             init: std::sync::Arc::new(f.into()),
//         }
//     }
// }

// impl<T> Deref for StaticRef<T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         self.cell.get_or_init(self.init)
//     }
// }

/// Returns a list of `Mod`s publled from an index based on the dep stings
/// from Thunderstore
///
/// # Errors
/// - A dependency string isn't formatted like `author-name`
/// - A dependency string isn't present in the index
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
///
/// # Errors
/// - The path cannot be canonicalized (broken symlinks)
/// - The path is not a directory
/// - There is no `enabledmods.json` file in the provided directory
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
///
/// # Errors
/// - The path cannot be canonicalized
/// - IO Errors
/// - Improperly formatted JSON files
pub fn find_mods(dir: impl AsRef<Path>) -> Result<Vec<InstalledMod>, ThermiteError> {
    let mut res = vec![];
    let dir = dir.as_ref().canonicalize()?;
    debug!("Finding mods in '{}'", dir.display());
    for child in dir.read_dir()? {
        let child = child?;
        if !child.file_type()?.is_dir() {
            debug!("Skipping file {}", child.path().display());
            continue;
        }
        // let path = child.path().join("mod.json");
        // let mod_json = if path.try_exists()? {
        //     let raw = fs::read_to_string(&path)?;
        //     let Ok(parsed) = json5::from_str(&raw) else {
        //         error!("Error parsing {}", path.display());
        //         continue;
        //     };
        //     parsed
        // } else {
        //     continue;
        // };
        let path = child.path().join("manifest.json");
        let manifest = if path.try_exists()? {
            let raw = fs::read_to_string(&path)?;
            let Ok(parsed) = serde_json::from_str(&raw) else {
                error!("Error parsing {}", path.display());
                continue;
            };
            parsed
        } else {
            continue;
        };

        if let Some(submods) = get_submods(&manifest, child.path()) {
            debug!(
                "Found {} submods in {}",
                submods.len(),
                child.path().display()
            );
            trace!("{:#?}", submods);
            let modstring =
                parse_modstring(child.file_name().to_str().ok_or(ThermiteError::UTF8Error)?)?;
            res.append(
                &mut submods
                    .into_iter()
                    .map(|mut m| {
                        m.author = modstring.0.clone();

                        m
                    })
                    .collect(),
            );
        } else {
            debug!("No mods in {}", child.path().display());
        }
    }

    Ok(res)
}

fn get_submods(manifest: &Manifest, dir: impl AsRef<Path>) -> Option<Vec<InstalledMod>> {
    let dir = dir.as_ref();
    debug!("Searching for submods in {}", dir.display());
    if !dir.is_dir() {
        debug!("Wasn't a directory, aborting");
        return None;
    }

    let mut mods = vec![];
    for child in dir.read_dir().ok()? {
        let Ok(child) = child else { continue };
        match child.file_type() {
            Ok(ty) => {
                if ty.is_dir() {
                    let Some(mut next) = get_submods(manifest, child.path()) else { continue };
                    mods.append(&mut next);
                } else {
                    trace!("Is file {:?} mod.json?", child.file_name());
                    if child.file_name() == "mod.json" {
                        trace!("Yes");
                        let Ok(file) = fs::read_to_string(child.path()) else { continue };
                        if let Ok(mod_json) = json5::from_str(&file) {
                            mods.push(InstalledMod {
                                author: String::new(),
                                manifest: manifest.clone(),
                                mod_json,
                                path: dir.to_path_buf(),
                            });
                        } else {
                            error!("Error parsing JSON in {}", child.path().display());
                        }
                    } else {
                        trace!("No");
                    }
                }
            }
            Err(e) => {
                error!("Error {e}");
            }
        }
    }

    if mods.is_empty() {
        None
    } else {
        Some(
            mods.into_iter()
                .map(|mut m| {
                    if m.path.ends_with("/mods") {
                        m.path.pop();
                    }

                    m
                })
                .collect(),
        )
    }
}

lazy_static! {
    pub static ref RE: Regex = Regex::new(r"^(\w+)-(\w+)-(\d+\.\d+\.\d+)$").unwrap();
}

pub fn parse_modstring(input: impl AsRef<str>) -> Result<ModString, ThermiteError> {
    debug!("Parsing modstring {}", input.as_ref());
    if let Some(captures) = RE.captures(input.as_ref()) {
        let author = captures
            .get(1)
            .ok_or_else(|| ThermiteError::NameError(input.as_ref().into()))?
            .as_str()
            .to_owned();

        let name = captures
            .get(2)
            .ok_or_else(|| ThermiteError::NameError(input.as_ref().into()))?
            .as_str()
            .to_owned();

        let version = captures.get(3).map(|v| v.as_str().to_string());

        Ok((author, name, version))
    } else {
        Err(ThermiteError::NameError(input.as_ref().into()))
    }
}

#[inline]
#[must_use]
pub fn validate_modstring(input: impl AsRef<str>) -> bool {
    RE.is_match(input.as_ref())
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
            .ok_or_else(|| ThermiteError::UnknownError("Malformed location URL".into()))?
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

#[cfg(test)]
mod test {
    use std::{collections::BTreeMap, path::PathBuf};

    use crate::model::Mod;

    use super::{resolve_deps, TempDir};

    const TEST_FOLDER: &str = "./test";

    #[test]
    fn temp_dir_deletes_on_drop() {
        {
            let temp_dir = TempDir::create(TEST_FOLDER);
            assert!(temp_dir.is_ok());

            if let Ok(dir) = temp_dir {
                let Ok(exists) = dir.try_exists() else { panic!("Unable to check if temp dir exists") };
                assert!(exists);
            }
        }

        let path = PathBuf::from(TEST_FOLDER);
        let Ok(exists) = path.try_exists() else { panic!("Unable to check if temp dir exists") };
        assert!(!exists);
    }

    #[test]
    fn reolve_dependencies() {
        let test_index: &[Mod] = &[Mod {
            name: "test".into(),
            latest: "0.1.0".into(),
            upgradable: false,
            global: false,
            installed: false,
            versions: BTreeMap::new(),
            author: "Foo".into(),
        }];

        let test_deps = &["foo-test-0.1.0"];

        let res = resolve_deps(test_deps, test_index);

        assert!(res.is_ok());
        assert_eq!(res.unwrap()[0], test_index[0]);
    }

    #[test]
    fn fail_resolve_bad_deps() {
        let test_index: &[Mod] = &[Mod {
            name: "test".into(),
            latest: "0.1.0".into(),
            upgradable: false,
            global: false,
            installed: false,
            versions: BTreeMap::new(),
            author: "Foo".into(),
        }];

        let test_deps = &["foo-test@0.1.0"];

        let res = resolve_deps(test_deps, test_index);

        assert!(res.is_err());

        let test_deps = &["foo-bar-0.1.0"];

        let res = resolve_deps(test_deps, test_index);

        assert!(res.is_err());
    }
}
