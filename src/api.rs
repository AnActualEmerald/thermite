use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::ThermiteError,
    model::{Mod, ModVersion},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PackageListing {
    name: String,
    owner: String,
    versions: Vec<PackageVersion>,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PackageVersion {
    dependencies: Vec<String>,
    description: String,
    download_url: String,
    file_size: u64,
    version_number: String,

    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

/// # Errors
/// * IO Erros
/// * Unexpected response format from thunderstore
pub fn get_package_index() -> Result<Vec<Mod>, ThermiteError> {
    let raw = ureq::get("https://northstar.thunderstore.io/c/northstar/api/v1/package/")
        .set("accept", "application/json")
        .call()?;
    let parsed: Vec<PackageListing> = serde_json::from_str(&raw.into_string()?)?;
    let index = map_response(&parsed);

    Ok(index)
}

fn map_response(res: &[PackageListing]) -> Vec<Mod> {
    res.iter()
        .map(|e| {
            let versions = &e.versions;
            let latest = versions[0].clone();
            let mut urls = BTreeMap::new();

            for v in versions {
                urls.insert(
                    v.version_number.clone(),
                    ModVersion {
                        name: e.name.clone(),
                        version: v.version_number.clone(),
                        desc: v.description.clone(),
                        file_size: v.file_size,
                        deps: v
                            .dependencies
                            .iter()
                            .filter(|e| !e.contains("northstar-Northstar"))
                            .cloned()
                            .collect::<Vec<String>>(),
                        installed: false,
                        global: false,
                        url: v.download_url.clone(),
                    },
                );
            }

            Mod {
                name: e.name.clone(),
                author: e.owner.clone(),
                latest: latest.version_number,
                versions: urls,
                installed: false,
                global: false,
                upgradable: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod test {
    use std::collections::{BTreeMap, HashMap};

    use crate::model::{Mod, ModVersion};

    use super::{PackageListing, PackageVersion, map_response, get_package_index};

    #[test]
    fn get_packages_from_tstore() {
        let index = get_package_index();
        assert!(index.is_ok());
        let index = index.unwrap();
        assert!(!index.is_empty());
        let mut deps = 0;
        for f in index {
            for d in f.versions.get(&f.latest).unwrap().deps.iter() {
                assert_ne!(d, "northstar-Northstar");
                deps += 1;
            }
        }
    
        assert_ne!(0, deps);
    }
    
    #[test]
    fn map_thunderstore_response() {
        let test_data = [PackageListing {
            name: "Foo".into(),
            owner: "Bar".into(),
            versions: vec![PackageVersion {
                dependencies: vec!["something".into()],
                description: "Test".into(),
                download_url: "localhost".into(),
                file_size: 420,
                version_number: "0.1.0".into(),
                _extra: HashMap::new()
            }],
            _extra: HashMap::new(),
        }];

        let expected = vec![Mod {
            name: "Foo".into(),
            author: "Bar".into(),
            latest: "0.1.0".into(),
            installed: false,
            upgradable: false, 
            global: false,
            versions: BTreeMap::from([("0.1.0".into(), ModVersion {
                name: "Foo".into(),
                version: "0.1.0".into(),
                url: "localhost".into(),
                desc: "Test".into(),
                deps: vec!["something".into()],
                installed: false,
                global: false,
                file_size: 420

            })])
        }];

        let res = map_response(&test_data);
        assert!(!res.is_empty());
        assert_eq!(res[0], expected[0]);
    }
}