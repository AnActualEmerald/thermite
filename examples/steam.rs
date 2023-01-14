use thermite::prelude::*;

fn main() {
    let libraries = steam_libraries();
    println!("{:#?}", libraries);
    let titanfall = titanfall();
    println!("{:#?}", titanfall);
}
