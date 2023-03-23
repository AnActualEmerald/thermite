use std::{fs, io::Cursor, path::Path};

use thermite::prelude::*;

fn main() {
    let index = get_package_index().unwrap();
    let Some(utils) = index
        .iter()
        .find(|v| v.name.to_lowercase() == "server_utilities") else {
            println!("Failed to find mod");
            return;
    };

    let mut buffer = vec![];
    download(&mut buffer, &utils.get_latest().unwrap().url).unwrap();

    //install_mod will panic if the directory doesn't exist
    if !Path::new("mods").try_exists().unwrap() {
        fs::create_dir("mods").unwrap();
    }
    install_mod("Fifty", Cursor::new(buffer), "mods").unwrap();
}
