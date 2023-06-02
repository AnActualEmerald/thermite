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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct ModJSON {
    pub name: String,
    pub description: String,
    pub version: String,
    pub load_priority: Option<i32>,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    #[must_use]
    pub fn get_latest(&self) -> Option<&ModVersion> {
        self.versions.get(&self.latest)
    }

    #[must_use]
    pub fn get_version(&self, version: impl AsRef<str>) -> Option<&ModVersion> {
        self.versions.get(version.as_ref())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    #[must_use]
    pub fn file_size_string(&self) -> String {
        if self.file_size / 1_000_000 >= 1 {
            let size = self.file_size / 1_048_576;

            format!("{size:.2} MB")
        } else {
            let size = self.file_size / 1024;
            format!("{size:.2} KB")
        }
    }
}

impl From<&Self> for ModVersion {
    fn from(value: &Self) -> Self {
        value.clone()
    }
}

impl AsRef<Self> for ModVersion {
    fn as_ref(&self) -> &Self {
        self
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version_number: String,
    pub website_url: String,
    pub description: String,
    pub dependencies: Vec<String>,
}

// enabledmods.json

/// Represents an enabledmods.json file
/// Automatically writes any changes made when dropped (call `dont_save` to disable)
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

            if self.do_save && hash != self.hash {
                if let Err(e) = self.save() {
                    error!(
                        "Encountered error while saving enabled_mods.json to {}:\n {}",
                        self.path.as_ref().unwrap().display(),
                        e
                    );
                } else {
                    debug!("Wrote file at {}", self.path.as_ref().unwrap().display());
                }
            }
        }
    }
}

impl EnabledMods {
    /// Attempts to read an `EnabledMods` from the path
    /// 
    /// # Errors
    /// - The file doesn't exist
    /// - The file isn't formatted properly
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ThermiteError> {
        let raw = fs::read_to_string(path)?;

        json5::from_str(&raw).map_err(|e| e.into())

    }

    /// Returns a default `EnabledMods` with the path property set
    pub fn default_with_path(path: impl AsRef<Path>) -> Self {
        let mut s = Self::default();
        s.path = Some(path.as_ref().to_path_buf());
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
    /// # Errors
    /// - If the path isn't set
    /// - If there is an IO error
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
    ///
    /// # Errors
    /// - If there is an IO error
    pub fn save_with_path(&mut self, path: impl AsRef<Path>) -> Result<(), ThermiteError> {
        self.path = Some(path.as_ref().to_owned());
        self.save()
    }

    /// Path the file will be written to
    #[must_use]
    pub const fn path(&self) -> Option<&PathBuf> {
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
    pub fn get(&self, name: impl AsRef<str>) -> Option<bool> {
        if CORE_MODS.contains(&name.as_ref()) {
            Some(match name.as_ref() {
                "Northstar.Client" => self.client,
                "Northstar.Custom" => self.custom,
                "Northstar.CustomServers" => self.servers,
                _ => unimplemented!(),
            })
        } else {
            self.mods.get(name.as_ref()).copied()
        }
    }

    /// Updates or inserts a mod's state
    pub fn set(&mut self, name: impl AsRef<str>, val: bool) -> Option<bool> {
        if CORE_MODS.contains(&name.as_ref().to_lowercase().as_str()) {
            let prev = self.get(&name);
            match name.as_ref().to_lowercase().as_str() {
                "northstar.client" => self.client = val,
                "northstar.custom" => self.custom = val,
                "northstar.customservers" => self.servers = val,
                _ => unimplemented!(),
            }
            prev
        } else {
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::core::utils::TempDir;

    use super::{ModJSON, Manifest, EnabledMods};

    const TEST_MOD_JSON: &str = r#"{
        "Name": "Test",
        "Description": "Test",
        "Version": "0.1.0",
        "LoadPriority": 1,
        "RequiredOnClient": false,
        "ConVars": [],
        "Scripts": [],
        "Localisation": []
    }"#;

    #[test]
    fn serialize_mod_json() {
        let test_data = ModJSON {
            name: "Test".into(),
            description: "Test".into(),
            version: "0.1.0".into(),
            load_priority: 1.into(),
            required_on_client: false.into(),
            con_vars: vec![],
            scripts: vec![],
            localisation: vec![],
            _extra: HashMap::new()
        };

        let ser = json5::to_string(&test_data);

        assert!(ser.is_ok());
    }

    #[test]
    fn deserialize_mod_json() {
        let test_data = ModJSON {
            name: "Test".into(),
            description: "Test".into(),
            version: "0.1.0".into(),
            load_priority: 1.into(),
            required_on_client: false.into(),
            con_vars: vec![],
            scripts: vec![],
            localisation: vec![],
            _extra: HashMap::new()
        };

        let de = json5::from_str::<ModJSON>(TEST_MOD_JSON);

        assert!(de.is_ok());
        assert_eq!(test_data, de.unwrap());

    }
    
    const TEST_MANIFEST: &str =  r#"{
        "name": "Test",
        "version_number": "0.1.0",
        "website_url": "https://example.com",
        "description": "Test",
        "dependencies": []
    }"#;

    #[test]
    fn deserialize_manifest() {
        let expected = Manifest {
            name: "Test".into(),
            version_number: "0.1.0".into(),
            website_url: "https://example.com".into(),
            description: "Test".into(),
            dependencies: vec![]
        };

        let de = json5::from_str(TEST_MANIFEST);

        assert!(de.is_ok());
        assert_eq!(expected, de.unwrap());
    }

    #[test]
    fn save_enabled_mods_on_drop() {
        let dir = TempDir::create("./test_save_enabled_mods").expect("Unable to create temp dir");
        let path = dir.join("enabled_mods.json");
        {
            let mut mods = EnabledMods::default_with_path(&path);
            mods.set("TestMod", false);
        }

        let mods = EnabledMods::load(&path);

        assert!(mods.is_ok());

        let test_mod = mods.unwrap().get("TestMod");
        assert!(test_mod.is_some());
        // this value should be false, so we assert the inverse
        assert!(!test_mod.unwrap());
    }

    #[test]
    fn disable_enabled_mods_autosave() {
        let dir = TempDir::create("./test_save_enabled_mods").expect("Unable to create temp dir");
        let path = dir.join("enabled_mods.json");
        {
            let mut mods = EnabledMods::default_with_path(&path);
            mods.set("TestMod", false);
            mods.dont_save();
        }

        let mods = EnabledMods::load(&path);

        assert!(mods.is_err());
    }

    #[test]
    fn enabled_mods_manual_save() {
        let dir = TempDir::create("./test_save_enabled_mods").expect("Unable to create temp dir");
        let path = dir.join("enabled_mods.json");
        {
            let mut mods = EnabledMods::default();
            mods.set("TestMod", false);
            mods.dont_save();
            mods.save_with_path(&path).expect("Unable to save enabled mods");
        }

        let mods = EnabledMods::load(&path);

        assert!(mods.is_ok());
        
        let test_mod = mods.unwrap().get("TestMod");
        
        assert!(test_mod.is_some());
        assert!(!test_mod.unwrap());

    }
}
