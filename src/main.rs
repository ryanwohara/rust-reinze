mod application;
mod plugins;

extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use std::fs::read_dir;
use std::path::Path;
use tokio;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() {
    let mut threads = vec![];

    for entry in read_dir(Path::new("conf/")).unwrap() {
        let path = entry.unwrap().path();
        let str = path.to_str().unwrap().to_string();

        if path.is_file() && str.ends_with(".toml") {
            let string = str.to_string();
            let thread = tokio::spawn(async move { application::run(&str.to_owned()).await });
            threads.push(Thread { thread, string });
        }
    }

    loop {
        let container = threads
            .iter()
            .map(|thread| {
                if thread.thread.is_finished() {
                    Some(thread.string.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<Option<String>>>();
        threads.retain(|thread| thread.thread.is_finished());
        container.iter().for_each(|str| {
            if str.is_some() {
                let string = str.as_ref().unwrap().to_string();
                threads.push(Thread {
                    string: string.to_string(),
                    thread: tokio::spawn(async move { application::run(string).await }),
                })
            }
        });
    }
}

struct Thread {
    string: String,
    thread: JoinHandle<()>,
}
