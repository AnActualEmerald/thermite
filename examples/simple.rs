use std::{fs::File, io::Write};

use futures_util::StreamExt;
use reqwest::Client;
use thermite::{api::get_package_index, prelude::*};

#[tokio::main]
async fn main() {
    let index = get_package_index().await.unwrap();
    let Some(utils) = index
        .iter()
        .find(|v| v.name.to_lowercase() == "server_utilities") else {
    println!("Failed to find mod");
    return;
};

    let client = Client::new();
    let res = client
        .get(utils.get_latest().unwrap().url.clone())
        .send()
        .await
        .unwrap();

    {
        //start download in chunks
        let mut file = File::create("utils.zip").unwrap();
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item.unwrap();
            file.write_all(&chunk).unwrap();
        }
        file.flush().unwrap();
    }

    let file = File::open("utils.zip").unwrap();
    //install_mod will panic if the directory doesn't exist
    std::fs::create_dir("mods").unwrap();
    install_mod("Fifty", &file, "mods").unwrap();
}
