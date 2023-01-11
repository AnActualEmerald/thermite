use std::{fs, path::Path};

use thermite::prelude::*;

#[tokio::main]
async fn main() {
    let index = get_package_index().await.unwrap();
    let Some(utils) = index
        .iter()
        .find(|v| v.name.to_lowercase() == "server_utilities") else {
            println!("Failed to find mod");
            return;
    };

    let file = download_file(&utils.get_latest().unwrap().url, "utils.zip")
        .await
        .unwrap();

    //install_mod will panic if the directory doesn't exist
    if !Path::new("mods").try_exists().unwrap() {
        fs::create_dir("mods").unwrap();
    }
    install_mod("Fifty", &file, "mods").unwrap();
}
