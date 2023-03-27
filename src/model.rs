use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap},
    hash::{Hash, Hasher},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, error};

use crate::{error::ThermiteError, CORE_MODS};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct ModJSON {
    pub name: String,
    pub description: String,
    pub version: String,
    pub load_priotity: Option<i32>,
    pub required_on_client: Option<bool>,
    #[serde(default)]
    pub con_vars: Vec<Value>,
    #[serde(default)]
    pub scripts: Vec<Value>,
    #[serde(default)]
    pub localisation: Vec<String>,
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mod {
    pub name: String,
    ///The latest version of the mod
    pub latest: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub upgradable: bool,
    #[serde(default)]
    pub global: bool,
    ///A map of each version of a mod
    pub versions: BTreeMap<String, ModVersion>,
    pub author: String,
}

impl Mod {
    pub fn get_latest(&self) -> Option<&ModVersion> {
        self.versions.get(&self.latest)
    }

    pub fn get_version(&self, version: impl AsRef<str>) -> Option<&ModVersion> {
        self.versions.get(version.as_ref())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModVersion {
    pub name: String,
    pub version: String,
    pub url: String,
    pub desc: String,
    pub deps: Vec<String>,
    pub installed: bool,
    pub global: bool,
    pub file_size: u64,
}

impl ModVersion {
    pub fn file_size_string(&self) -> String {
        if self.file_size / 1_000_000 >= 1 {
            let size = self.file_size as f64 / 1_048_576f64;

            format!("{size:.2} MB")
        } else {
            let size = self.file_size as f64 / 1024f64;
            format!("{size:.2} KB")
        }
    }
}

impl From<&ModVersion> for ModVersion {
    fn from(value: &ModVersion) -> Self {
        value.clone()
    }
}

impl AsRef<ModVersion> for ModVersion {
    fn as_ref(&self) -> &ModVersion {
        self
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

// enabledmods.json

/// Represents an enabledmods.json file
/// Automatically writes any changes made when dropped (call dont_save to disable)
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
    hash: u64,
    ///Path to the file to read & write
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(skip)]
    do_save: bool,
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
            path: None,
            do_save: true,
        }
    }
}

impl Drop for EnabledMods {
    fn drop(&mut self) {
        if self.path.is_some() {
            let hash = {
                let mut hasher = DefaultHasher::new();
                self.hash(&mut hasher);
                hasher.finish()
            };

            if hash != self.hash {
                if let Err(e) = self.save() {
                    error!(
                        "Encountered error while saving enabled_mods.json to {}:\n {}",
                        self.path.as_ref().unwrap().display(),
                        e
                    );
                } else {
                    debug!("Wrote file at {}", self.path.as_ref().unwrap().display())
                }
            }
        }
    }
}

impl EnabledMods {
    /// Returns a default EnabledMods with the path property set
    pub fn default_with_path(path: impl Into<PathBuf>) -> Self {
        let mut s = Self::default();
        s.path = Some(path.into());
        s
    }

    /// Don't attempt to write the file when dropped
    pub fn dont_save(&mut self) {
        self.do_save = false;
    }

    /// Do attempt to write the file when dropped
    pub fn do_save(&mut self) {
        self.do_save = true;
    }

    /// Saves the file using the path it was loaded from
    ///
    /// Returns an error if the path isn't set
    pub fn save(&self) -> Result<(), ThermiteError> {
        let parsed = serde_json::to_string_pretty(self)?;
        if let Some(path) = &self.path {
            if let Some(p) = path.parent() {
                fs::create_dir_all(p)?;
            }

            fs::write(path, parsed)?;
            Ok(())
        } else {
            Err(ThermiteError::MissingPath)
        }
    }

    /// Saves the file using the provided path
    pub fn save_with_path(&mut self, path: impl AsRef<Path>) -> Result<(), ThermiteError> {
        self.path = Some(path.as_ref().to_owned());
        self.save()
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn set_path(&mut self, path: impl Into<Option<PathBuf>>) {
        self.path = path.into();
    }
    
    /// Returns the current state of a mod
    ///
    /// # Warning
    /// Returns `true` if a mod is missing from the file
    pub fn is_enabled(&self, name: impl AsRef<str>) -> bool {
        *self.mods.get(name.as_ref()).unwrap_or(&true)
    }

    /// Get the current state of a mod if it exists
    pub fn get(&self , name: impl AsRef<str>) -> Option<bool> {
        if CORE_MODS.contains(&name.as_ref()) {
            Some(match name.as_ref() {
                "Northstar.Client" => self.client,
                "Northstar.Custom" => self.custom,
                "Northstar.CustomServers" => self.servers,
                _ => unimplemented!(),
            })
        }else {
            self.mods.get(name.as_ref()).copied()
        }
    }

    /// Updates or inserts a mod's state
    pub fn set(&mut self , name: impl AsRef<str>, val: bool) -> Option<bool> {
        if CORE_MODS.contains(&name.as_ref().to_lowercase().as_str()) {
            let prev = self.get(&name);
            match name.as_ref().to_lowercase().as_str() {
                "northstar.client" => self.client = val,
                "northstar.custom" => self.custom = val,
                "northstar.customservers" => self.servers = val,
                _ => unimplemented!(),
            }
            prev
        }else {
            self.mods.insert(name.as_ref().to_string(), val)
        }
    }


    
}

/// Represents an installed package
#[derive(Debug, Clone)]
pub struct InstalledMod {
    pub manifest: Manifest,
    pub mod_json: ModJSON,
    pub author: String,
    pub path: PathBuf,
}
