use std::collections::{BTreeMap, HashMap};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::ThermiteError,
    model::{Mod, ModVersion},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PackageListing {
    name: String,
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

pub async fn get_package_index() -> Result<Vec<Mod>, ThermiteError> {
    let client = Client::new();
    let raw = client
        .get("https://northstar.thunderstore.io/c/northstar/api/v1/package/")
        .header("accept", "application/json")
        .send()
        .await?;
    if raw.status().is_success() {
        let parsed: Vec<PackageListing> = serde_json::from_str(&raw.text().await.unwrap())?;
        let index = map_response(&parsed);

        Ok(index)
    } else {
        Err(ThermiteError::MiscError(raw.status().to_string()))
    }
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
                            .filter(|e| *e == "northstar-Northar")
                            .cloned()
                            .collect::<Vec<String>>(),
                        url: v.download_url.clone(),
                    },
                );
            }

            Mod {
                name: e.name.clone(),
                latest: latest.version_number,
                versions: urls,
                installed: false,
                global: false,
                upgradable: false,
            }
        })
        .collect()
}
