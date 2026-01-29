mod application;
mod plugins;

extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use std::fs::read_dir;
use std::path::Path;
use tokio;

#[tokio::main]
async fn main() {
    for entry in read_dir(Path::new("conf/")).unwrap() {
        let path = entry.unwrap().path();
        let str = path.to_str().unwrap().to_string();

        if path.is_file() && str.ends_with(".toml") {
            tokio::spawn(async move { application::run(&str).await });
        }
    }

    loop {}
}
