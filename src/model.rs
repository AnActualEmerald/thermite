use log::{debug, trace, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
};

use crate::{core::utils, error::ThermiteError};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mod {
    pub name: String,
    pub version: String,
    pub url: String,
    pub desc: String,
    pub deps: Vec<String>,
    pub file_size: i64,
    #[serde(default)]
    pub installed: bool,
    pub global: bool,
    #[serde(default)]
    pub upgradable: bool,
}

impl Mod {
    pub fn file_size_string(&self) -> String {
        if self.file_size / 1_000_000 >= 1 {
            let size = self.file_size as f64 / 1_048_576f64;

            format!("{:.2} MB", size)
        } else {
            let size = self.file_size as f64 / 1024f64;
            format!("{:.2} KB", size)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct LocalMod {
    pub package_name: String,
    pub version: String,
    pub mods: Vec<SubMod>,
    //TODO: Implement local dep tracking
    pub depends_on: Vec<String>,
    pub needed_by: Vec<String>,
}

impl LocalMod {
    pub fn flatten_paths(&self) -> Vec<&PathBuf> {
        self.mods.iter().map(|m| &m.path).collect()
    }

    pub fn any_disabled(&self) -> bool {
        self.mods.iter().any(|m| m.disabled())
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct SubMod {
    pub path: PathBuf,
    pub name: String,
}

impl SubMod {
    pub fn new(name: &str, path: &Path) -> Self {
        SubMod {
            name: name.to_string(),
            path: path.to_owned(),
        }
    }

    pub fn disabled(&self) -> bool {
        self.path
            .components()
            .any(|f| f.as_os_str() == OsStr::new(".disabled"))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version_number: String,
    pub website_url: String,
    pub description: String,
    pub dependencies: Vec<String>,
}

/// Index of mods installed locally
///
/// Will save itself when it goes out of scope
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct LocalIndex {
    #[serde(default)]
    pub mods: BTreeMap<String, LocalMod>,
    #[serde(default)]
    pub linked: BTreeMap<String, LocalMod>,
    #[serde(skip)]
    path: PathBuf,
    #[serde(skip)]
    hash: u64,
}

impl Hash for LocalIndex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.mods.hash(state);
        self.linked.hash(state);
    }
}

impl LocalIndex {
    /// Load an existing RON-format index file
    /// # Params
    /// * path - Path to the index file to load
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ThermiteError> {
        let path = path.as_ref();
        if path.exists() {
            let raw = fs::read_to_string(path)?;
            let mut parsed = ron::from_str::<Self>(&raw)?;
            parsed.path = path.into();
            let hash = {
                let mut hasher = DefaultHasher::new();
                parsed.hash(&mut hasher);
                hasher.finish()
            };
            parsed.hash = hash;
            Ok(parsed)
        } else {
            Err(ThermiteError::MissingFile(path.into()))
        }
    }

    /// Tries to load an existing RON-format index file, or creates one if it doesn't exist
    /// # Params
    /// * path - Path to file to try to load
    pub fn load_or_create(path: impl AsRef<Path>) -> Self {
        match Self::load(path.as_ref()) {
            Ok(s) => s,
            Err(_) => {
                debug!("Creating index at {}", path.as_ref().display());
                let s = Self::create(path.as_ref());
                s.save().unwrap();
                s
            }
        }
    }

    /// Create a new index file
    /// # Params
    /// * path - Path to create the file at
    pub fn create(path: &Path) -> Self {
        let mut ind = Self::default();
        ind.path = path.into();

        ind
    }

    /// Save the index file
    ///
    /// This function will be called when the `LocalIndex` is dropped. It shouldn't need to be called manually.
    pub fn save(&self) -> Result<(), ThermiteError> {
        let parsed = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())?;
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&self.path, &parsed).map_err(|e| e.into())
    }

    /// Returns the parent directory of the index file if available,
    /// or a default `PathBuf` otherwise
    pub fn parent_dir(&self) -> PathBuf {
        if let Some(p) = self.path.parent() {
            p.to_path_buf()
        } else {
            PathBuf::default()
        }
    }

    /// Calls `LocalIndex::save` only if the index was modified
    ///
    /// Returns whether or not the index was written to disk
    pub fn save_if_changed(&self) -> bool {
        let hash = {
            let mut hasher = DefaultHasher::new();
            self.hash(&mut hasher);
            hasher.finish()
        };

        trace!("Old hash: {}\nNew hash: {}", self.hash, hash);
        if hash != self.hash {
            self.save().unwrap();
            true
        } else {
            false
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }

    pub fn get_mod(&self, name: &str) -> Option<&LocalMod> {
        if self.mods.contains_key(name) {
            self.mods.get(name)
        } else if self.linked.contains_key(name) {
            self.linked.get(name)
        } else {
            None
        }
    }

    pub fn get_sub_mod(&self, name: &str) -> Option<&SubMod> {
        for m in self.mods.values().chain(self.linked.values()) {
            if let Some(m) = m.mods.iter().find(|e| e.name == name) {
                return Some(m);
            }
        }

        None
    }
}

impl Drop for LocalIndex {
    fn drop(&mut self) {
        if self.save_if_changed() {
            debug!("Saved index at {}", self.path().display());
        }
    }
}

#[derive(Debug, Clone)]
struct CachedMod {
    name: String,
    version: String,
    path: PathBuf,
}

impl PartialEq for CachedMod {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version
    }
}

impl CachedMod {
    fn new(name: &str, version: &str, path: &Path) -> Self {
        CachedMod {
            name: name.to_string(),
            version: version.to_string(),
            path: path.to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cache {
    re: Regex,
    pkgs: Vec<CachedMod>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            re: Regex::new("").unwrap(),
            pkgs: vec![],
        }
    }
}

impl Cache {
    pub fn build(dir: &Path) -> Result<Self, ThermiteError> {
        let cache = fs::read_dir(dir)?;
        let re = Regex::new(r"(.+)[_-](\d\.\d\.\d)(\.zip)?").expect("Unable to create cache regex");
        let mut pkgs = vec![];
        for e in cache.flatten() {
            if !e.path().is_dir() {
                debug!("Reading {} into cache", e.path().display());
                let file_name = e.file_name();
                if let Some(c) = re.captures(file_name.to_str().unwrap()) {
                    let name = c.get(1).unwrap().as_str().trim();
                    let ver = c.get(2).unwrap().as_str().trim();
                    pkgs.push(CachedMod::new(name, ver, dir));
                    debug!("Added {} version {} to cache", name, ver);
                } else {
                    warn!(
                        "Unexpected filename in cache dir: {}",
                        file_name.to_str().unwrap()
                    );
                }
            }
        }
        Ok(Cache { pkgs, re })
    }

    ///Cleans all cached versions of a package except the version provided
    pub fn clean(&mut self, name: &str, version: &str) -> Result<bool, ThermiteError> {
        let mut res = false;

        for m in self
            .pkgs
            .clone()
            .into_iter()
            .filter(|e| e.name == name && e.version != version)
        {
            if let Some(index) = self.pkgs.iter().position(|e| e == &m) {
                utils::remove_file(&m.path)?;
                self.pkgs.swap_remove(index);
                res = true
            }
        }

        Ok(res)
    }

    ///Checks if a path is in the current cache
    pub fn check(&self, path: &Path) -> Option<File> {
        if self.has(path) {
            self.open_file(path)
        } else {
            None
        }
    }

    fn has(&self, path: &Path) -> bool {
        if let Some(name) = path.file_name() {
            if let Some(parts) = self.re.captures(name.to_str().unwrap()) {
                let name = parts.get(1).unwrap().as_str();
                let ver = parts.get(2).unwrap().as_str();
                if let Some(c) = self.pkgs.iter().find(|e| e.name == name) {
                    if c.version == ver {
                        return true;
                    }
                }
            }
        }
        false
    }

    #[inline(always)]
    fn open_file(&self, path: &Path) -> Option<File> {
        if let Ok(f) = OpenOptions::new().read(true).open(path) {
            Some(f)
        } else {
            None
        }
    }
}

// enabledmods.json

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnabledMods {
    #[serde(rename = "Northstar.Client")]
    pub client: bool,
    #[serde(rename = "Northstar.Custom")]
    pub custom: bool,
    #[serde(rename = "Northstar.CustomServers")]
    pub servers: bool,
    #[serde(flatten)]
    pub mods: BTreeMap<String, bool>,
    ///Hash of the file as it was loaded
    #[serde(skip)]
    hash: u64
    ///Path to the file to read & write
    #[serde(skip)]
    path: Option<PathBuf>
}

impl Hash for EnabledMods {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.client.hash(state);
        self.custom.hash(state);
        self.servers.hash(state);
        self.mods.hash(state);
    }
}

impl Default for EnabledMods {
    fn default() -> Self {
        Self {
            client: true,
            custom: true,
            servers: true,
            mods: BTreeMap::new(),
            hash: 0,
            path: None
        }
    }
}

impl Drop for EnabledMods {
    fn drop(&mut self) {
        if let Some(path) = self.path {
            let hash = {
                let mut hasher = DefaultHasher::new();
                self.hash(&mut hasher);
                hasher.finish()
            }

            if hash != self.hash {
                self.save().unwrap()
            }

        }
    }
}

impl EnabledMods {
    pub fn save(&self) -> Result<(), ThermiteError> {
        let parsed = serde_json::to_string_pretty(self)?;
        if let Some(p) = self.path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&self.path, &parsed).map_err(|e| e.into())
    }

    pub fn save_with_path(&mut self, path: impl AsRef<Path>) -> Result<(), ThermiteError> {
        self.path = path.as_ref().to_owned();
        self.save()
    }
}

