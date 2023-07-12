use std::{io::Cursor, path::Path};

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

    let target_dir = Path::new("packages").join(&utils.get_latest().unwrap().full_name);

    install_mod(Cursor::new(buffer), target_dir).unwrap();
}
