use thermite::prelude::*;

fn main() {
    let libraries = steam_libraries();
    println!("{:#?}", libraries);
    let titanfall = titanfall2_dir()
        .expect("This will fail if steam isn't present or titalfall 2 isn't installed");
    println!("{:#?}", titanfall);
}
