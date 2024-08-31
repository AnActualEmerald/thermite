use crate::error::ThermiteError;
use crate::model::EnabledMods;
use crate::model::InstalledMod;
use crate::model::Manifest;
use crate::model::Mod;

use lazy_static::lazy_static;
use regex::Regex;
use std::fmt::Debug;
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;

use tracing::trace;
use tracing::{debug, error};

pub(crate) type ModString = (String, String, String);

#[derive(Debug, Clone)]
pub(crate) struct TempDir {
    pub(crate) path: PathBuf,
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
                        m.author.clone_from(&modstring.0);

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
                    let Some(mut next) = get_submods(manifest, child.path()) else {
                        continue;
                    };
                    mods.append(&mut next);
                } else {
                    trace!("Is file {:?} mod.json?", child.file_name());
                    if child.file_name() == "mod.json" {
                        trace!("Yes");
                        let Ok(file) = fs::read_to_string(child.path()) else {
                            continue;
                        };
                        match json5::from_str(&file) {
                            Ok(mod_json) => mods.push(InstalledMod {
                                author: String::new(),
                                manifest: manifest.clone(),
                                mod_json,
                                path: dir.to_path_buf(),
                            }),
                            Err(e) => {
                                error!("Error parsing JSON in {}: {e}", child.path().display());
                            }
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
    pub static ref RE: Regex =
        Regex::new(r"^(\w+)-(\w+)-(\d+\.\d+\.\d+)$").expect("lazy compile regex");
}

/// Returns the parts of a `author-name-X.Y.Z` string in (`author`, `name`, `version`) order
///
/// # Errors
///
/// Returns a `NameError` if the input string is not in the correct format
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

        let version = captures
            .get(3)
            .ok_or_else(|| ThermiteError::NameError(input.as_ref().into()))?
            .as_str()
            .to_owned();

        Ok((author, name, version))
    } else {
        Err(ThermiteError::NameError(input.as_ref().into()))
    }
}

/// Checks that a string is in `author-name-X.Y.Z` format
#[inline]
#[must_use]
pub fn validate_modstring(input: impl AsRef<str>) -> bool {
    RE.is_match(input.as_ref())
}

#[cfg(feature = "steam")]
pub(crate) mod steam {
    use std::path::PathBuf;
    use steamlocate::SteamDir;

    use crate::TITANFALL2_STEAM_ID;

    /// Returns the path to the Steam installation if it exists
    #[must_use]
    #[inline]
    pub fn steam_dir() -> Option<PathBuf> {
        SteamDir::locate().map(|v| v.path)
    }

    /// Returns paths to all known Steam libraries
    #[must_use]
    pub fn steam_libraries() -> Option<Vec<PathBuf>> {
        let mut steamdir = SteamDir::locate()?;
        let folders = steamdir.libraryfolders();
        Some(folders.paths.clone())
    }

    /// Returns the path to the Titanfall installation if it exists
    #[must_use]
    pub fn titanfall() -> Option<PathBuf> {
        let mut steamdir = SteamDir::locate()?;
        Some(steamdir.app(&TITANFALL2_STEAM_ID)?.path.clone())
    }
}

#[cfg(all(target_os = "linux", feature = "proton"))]
//#[deprecated(since = "0.8.0", note = "Northstar Proton is no longer required")]
pub(crate) mod proton {
    use flate2::read::GzDecoder;
    use std::{
        io::{Read, Write},
        path::Path,
    };
    use tar::Archive;
    use tracing::debug;

    use crate::{
        core::manage::download,
        error::{Result, ThermiteError},
    };
    const BASE_URL: &str = "https://github.com/R2NorthstarTools/NorthstarProton/releases/";

    /// Returns the latest tag from the NorthstarProton repo
    ///
    /// # Errors
    /// * Network error
    /// * Unexpected URL format
    pub fn latest_release() -> Result<String> {
        let url = format!("{}latest", BASE_URL);
        let res = ureq::get(&url).call()?;
        let location = res.get_url();
        debug!("{url} redirected to {location}");

        Ok(location
            .split('/')
            .last()
            .ok_or_else(|| ThermiteError::UnknownError("Malformed location URL".into()))?
            .to_owned())
    }

    /// Convinience function for downloading a given tag from the NorthstarProton repo.
    /// If you have a URL already, just use `thermite::manage::download`
    pub fn download_ns_proton(tag: impl AsRef<str>, output: impl Write) -> Result<u64> {
        let url = format!(
            "{}download/{}/NorthstarProton{}.tar.gz",
            BASE_URL,
            tag.as_ref(),
            tag.as_ref().trim_matches('v')
        );
        download(output, url)
    }

    /// Extract the NorthstarProton tarball into a given directory.
    /// Only supports extracting to a filesystem path.
    ///
    /// # Errors
    /// * IO errors
    pub fn install_ns_proton(archive: impl Read, dest: impl AsRef<Path>) -> Result<()> {
        let mut tarball = Archive::new(GzDecoder::new(archive));
        tarball.unpack(dest)?;

        Ok(())
    }

    #[cfg(test)]
    mod test {
        use std::io::Cursor;

        use crate::core::utils::TempDir;

        use super::latest_release;

        #[test]
        fn get_latest_proton_version() {
            let res = latest_release();
            assert!(res.is_ok());
        }

        #[test]
        fn extract_proton() {
            let dir =
                TempDir::create(std::env::temp_dir().join("NSPROTON_TEST")).expect("temp dir");
            let archive = include_bytes!("test_media/NorthstarProton8-28.tar.gz");
            let cursor = Cursor::new(archive);
            let res = super::install_ns_proton(cursor, &dir);
            assert!(res.is_ok());

            let extracted = dir.join("NorthstarProton8-28.txt");
            assert!(extracted.exists());
            assert_eq!(
                std::fs::read_to_string(extracted).expect("read file"),
                "The real proton was too big to use as test media\n"
            );
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    };

    use crate::{error::ThermiteError, model::Mod};

    use super::{
        find_mods, get_enabled_mods, parse_modstring, resolve_deps, validate_modstring, TempDir,
    };

    #[test]
    fn temp_dir_deletes_on_drop() {
        let test_folder = "temp_dir";
        {
            let temp_dir = TempDir::create(test_folder);
            assert!(temp_dir.is_ok());

            if let Ok(dir) = temp_dir {
                let exists = dir
                    .try_exists()
                    .expect("Unable to check if temp dir exists");
                assert!(exists);
            }
        }

        let path = PathBuf::from(test_folder);
        let exists = path
            .try_exists()
            .expect("Unable to check if temp dir exists");
        assert!(!exists);
    }

    #[test]
    fn fail_find_enabledmods() {
        let test_folder = "fail_enabled_mods_test";
        let temp_dir = TempDir::create(test_folder).unwrap();
        if let Err(ThermiteError::MissingFile(path)) = get_enabled_mods(&temp_dir) {
            assert_eq!(
                *path,
                temp_dir.canonicalize().unwrap().join("enabledmods.json")
            );
        } else {
            panic!("enabledmods.json should not exist");
        }
    }

    #[test]
    fn fail_parse_enabledmods() {
        let test_folder = "parse_enabled_mods_test";
        let temp_dir = TempDir::create(test_folder).unwrap();
        fs::write(temp_dir.join("enabledmods.json"), b"invalid json").unwrap();
        if let Err(ThermiteError::JsonError(_)) = get_enabled_mods(temp_dir) {
        } else {
            panic!("enabledmods.json should not be valid json");
        }
    }

    #[test]
    fn pass_get_enabledmods() {
        let test_folder = "pass_enabled_mods_test";
        let temp_dir = TempDir::create(test_folder).unwrap();
        fs::write(temp_dir.join("enabledmods.json"), b"{}").unwrap();
        if let Ok(mods) = get_enabled_mods(temp_dir) {
            assert!(mods.client);
            assert!(mods.custom);
            assert!(mods.servers);
            assert!(mods.mods.is_empty());
        } else {
            panic!("enabledmods.json should be valid but empty");
        }
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
    fn dont_resolve_northstar_as_dependency() {
        let test_index: &[Mod] = &[Mod {
            name: "Northstar".into(),
            latest: "0.1.0".into(),
            upgradable: false,
            global: false,
            installed: false,
            versions: BTreeMap::new(),
            author: "Northstar".into(),
        }];

        let test_deps = &["Northstar-Northstar-0.1.0"];

        let res = resolve_deps(test_deps, test_index);

        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
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

    #[test]
    fn sucessfully_validate_modstring() {
        let test_string = "author-mod-0.1.0";
        assert!(validate_modstring(test_string));
    }

    #[test]
    fn fail_validate_modstring() {
        let test_string = "invalid";
        assert!(!validate_modstring(test_string));
    }

    #[test]
    fn successfully_parse_modstring() {
        let test_string = "author-mod-0.1.0";
        let res = parse_modstring(test_string);

        if let Ok(parsed) = res {
            assert_eq!(parsed, ("author".into(), "mod".into(), "0.1.0".into()));
        } else {
            panic!("Valid mod string failed to be parsed");
        }
    }

    #[test]
    fn fail_parse_modstring() {
        let test_string = "invalid";
        let res = parse_modstring(test_string);

        if let Err(ThermiteError::NameError(name)) = res {
            assert_eq!(name, test_string);
        } else {
            panic!("Invalid mod string didn't error");
        }
    }

    const MANIFEST: &str = r#"{
        "namespace": "northstar",
        "name": "Northstar",
        "description": "Titanfall 2 modding and custom server framework.",
        "version_number": "1.22.0",
        "dependencies": [],
        "website_url": ""
      }"#;

    const MOD_JSON: &str = r#"{
        "Name": "Yourname.Modname",
        "Description": "Woo yeah wooo!",
        "Version": "1.2.3",
     
        "LoadPriority": 0,
        "ConVars": [],
        "Scripts": [],
        "Localisation": []
     }"#;

    fn setup_mods(path: impl AsRef<Path>) {
        let root = path.as_ref().join("northstar-mod-1.2.3");
        fs::create_dir_all(&root).expect("create dir");
        fs::write(root.join("manifest.json"), MANIFEST).expect("write manifest");
        let _mod = root.join("RealMod");
        fs::create_dir_all(&_mod).expect("create dir");
        fs::write(_mod.join("mod.json"), MOD_JSON).expect("write mod.json");
    }

    #[test]
    fn discover_mods() {
        let dir = TempDir::create("./mod_discovery").expect("Temp dir");
        setup_mods(&dir);
        let res = find_mods(dir);

        if let Ok(mods) = res {
            assert_eq!(mods.len(), 1, "Should be one mod");
            assert_eq!(mods[0].manifest.name, "Northstar");
            assert_eq!(mods[0].author, "northstar");
            assert_eq!(mods[0].mod_json.name, "Yourname.Modname");
        } else {
            panic!("Mod discovery failed: {res:?}");
        }
    }
}
