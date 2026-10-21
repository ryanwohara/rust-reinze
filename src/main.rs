mod application;
mod plugins;
mod timers;

extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use common::author::cache::{color_ffi, init};
use std::fs::read_dir;
use std::path::Path;
use std::time::Duration;
use tokio;
use tokio::time;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    env_logger::init();
    let mut networks = vec![];
    init();

    for entry in read_dir(Path::new("conf/")).unwrap() {
        let path = entry.unwrap().path();
        let str = path.to_str().unwrap().to_string();

        if path.is_file() && str.ends_with(".toml") {
            networks.push(tokio::spawn(supervise(str)));
        }
    }

    for network in networks {
        network.await.ok();
    }
}

// `run` only returns by panicking; restart it so one network's crash doesn't
// leave that network offline while the others keep running.
async fn supervise(path: String) {
    loop {
        let path = path.clone();
        let handle = tokio::spawn(async move { application::run(path, color_ffi).await });

        if let Err(e) = handle.await {
            eprintln!("Client task ended ({}), restarting in 5 secs...", e);
        }

        time::sleep(Duration::from_secs(5)).await;
    }
}
