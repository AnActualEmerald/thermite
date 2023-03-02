use std::{fs, path::Path};
use std::time::Duration;

use thermite::prelude::*;

fn main() {
    let index = get_package_index().unwrap();
    let Some(utils) = index
        .iter()
        .find(|v| v.name.to_lowercase() == "server_utilities") else {
            println!("Failed to find mod");
            return;
    };

    let pb = indicatif::ProgressBar::new(utils.get_latest().unwrap().file_size)
    .with_style(indicatif::ProgressStyle::default_bar().progress_chars("->.").template("{msg} {wide_bar} {bytes}/{total_bytes}").unwrap())
    .with_message("Downloading Fifty.Server_Utilities");

    let file = download_file_with_progress(&utils.get_latest().unwrap().url, "utils.zip", |delta, _, _| {
        pb.inc(delta);
        //slow down the download to show off the progress bar
        //(you probably shouldn't do this in production)
        std::thread::sleep(Duration::from_millis(100));
    }).unwrap();

    pb.finish_with_message("Done!");

    //install_mod will panic if the directory doesn't exist
    if !Path::new("mods").try_exists().unwrap() {
        fs::create_dir("mods").unwrap();
    }
    install_mod("Fifty", &file, "mods").unwrap();
} 