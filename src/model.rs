use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use std::{
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    pub full_name: String,
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

/// Represents an enabledmods.json file. Core mods will default to `true` if not present when deserializing.
///
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnabledMods {
    #[serde(rename = "Northstar.Client", default = "default_mod_state")]
    pub client: bool,
    #[serde(rename = "Northstar.Custom", default = "default_mod_state")]
    pub custom: bool,
    #[serde(rename = "Northstar.CustomServers", default = "default_mod_state")]
    pub servers: bool,
    #[serde(flatten)]
    pub mods: BTreeMap<String, bool>,
    ///Path to the file to read & write
    #[serde(skip)]
    path: Option<PathBuf>,
}

fn default_mod_state() -> bool {
    true
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
            path: None,
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

        json5::from_str(&raw).map_err(Into::into)
    }

    /// Returns a default `EnabledMods` with the path property set
    pub fn default_with_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: Some(path.as_ref().to_path_buf()),
            ..Default::default()
        }
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
        self.mods.get(name.as_ref()).copied().unwrap_or(true)
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMod {
    pub manifest: Manifest,
    pub mod_json: ModJSON,
    pub author: String,
    pub path: PathBuf,
}

impl PartialOrd for InstalledMod {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// [InstalledMod]s are ordered by their author, then manifest name, then mod.json name
impl Ord for InstalledMod {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.author.cmp(&other.author) {
            std::cmp::Ordering::Equal => match self.manifest.name.cmp(&other.manifest.name) {
                std::cmp::Ordering::Equal => self.mod_json.name.cmp(&other.mod_json.name),
                ord => ord,
            },
            ord => ord,
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::core::utils::TempDir;

    use super::{EnabledMods, Manifest, ModJSON};

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
            _extra: HashMap::new(),
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
            _extra: HashMap::new(),
        };

        let de = json5::from_str::<ModJSON>(TEST_MOD_JSON);

        assert!(de.is_ok());
        assert_eq!(test_data, de.unwrap());
    }

    const TEST_MANIFEST: &str = r#"{
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
            dependencies: vec![],
        };

        let de = json5::from_str(TEST_MANIFEST);

        assert!(de.is_ok());
        assert_eq!(expected, de.unwrap());
    }

    #[test]
    fn save_enabled_mods() {
        let dir =
            TempDir::create("./test_autosave_enabled_mods").expect("Unable to create temp dir");
        let path = dir.join("enabled_mods.json");
        {
            let mut mods = EnabledMods::default_with_path(&path);
            mods.set("TestMod", false);
            mods.save().expect("Write enabledmods.json");
        }

        let mods = EnabledMods::load(&path);

        if let Err(e) = mods {
            panic!("Failed to load enabled_mods: {e}");
        }

        let test_mod = mods.unwrap().get("TestMod");
        assert!(test_mod.is_some());
        // this value should be false, so we assert the inverse
        assert!(!test_mod.unwrap());
    }
}
