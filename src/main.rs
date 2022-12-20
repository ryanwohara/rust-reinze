pub mod common;

extern crate reqwest;
extern crate select;

use anyhow::Result;
use futures::prelude::*;
use irc::client::prelude::*;
use libloading::{Library, Symbol};
use regex::Regex;
use std::fs;
use tokio;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut loaded_plugins = Vec::new();

    let plugins = fs::read_dir("plugins/").unwrap();

    for plugin in plugins {
        let plugin = plugin.unwrap();

        if match plugin.path().extension() {
            Some(ext) => ext,
            None => continue,
        } == "so"
        {
            println!("Loading plugin: {}", plugin.path().display());

            unsafe {
                // Load the dynamic library
                let lib = Library::new(plugin.path())?;

                // Get a reference to the `exported` function
                let exported: Symbol<
                    extern "C" fn(command: &str, query: &str) -> Result<Vec<String>, ()>,
                > = lib.get(b"exported\0")?;

                // Call the `exported` function
                let functions = exported("", "").unwrap();

                println!("Functions: {:?}", functions);

                let loaded_plugin: Plugin = Plugin {
                    name: plugin.path().to_str().unwrap().to_string(),
                    commands: functions,
                };

                loaded_plugins.push(loaded_plugin);
            }
        }
    }

    for plugin in &loaded_plugins {
        println!(".Plugin: {}", plugin.name);
        for command in &plugin.commands {
            println!("..Command: {}", command);
        }
    }

    // We can also load the Config at runtime via Config::load("path/to/config.toml")
    let config = Config {
        nickname: Some("RustKick".to_string()),
        server: Some("fiery.swiftirc.net".to_string()),
        channels: vec!["#asdfghj,#rshelp".to_string()],
        ..Config::default()
    };

    let mut client = Client::from_config(config).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    while let Some(message) = stream.next().await.transpose()? {
        print!("{}", message);

        if let Command::PRIVMSG(ref _channel, ref _message) = message.command {
            if let Some(target) = message.response_target() {
                let re = Regex::new(r"^[-+](\w+)\s*(.*)").unwrap();
                let matched = re.captures(_message);
                if matched.is_some() {
                    let cmd = matched.as_ref().unwrap().get(1).unwrap().as_str();
                    let param = matched.as_ref().unwrap().get(2).unwrap().as_str();

                    for plugin in &loaded_plugins {
                        if plugin.commands.contains(&cmd.to_string()) {
                            unsafe {
                                let lib = match Library::new(plugin.name.clone()) {
                                    Ok(lib) => lib,
                                    Err(e) => {
                                        println!("Error loading plugin: {}", e);
                                        continue;
                                    }
                                };

                                let exported: Symbol<
                                    extern "C" fn(
                                        command: &str,
                                        query: &str,
                                    )
                                        -> Result<Vec<String>, ()>,
                                > = lib.get(b"exported\0")?;

                                let result = match exported(cmd, param) {
                                    Ok(result) => result,
                                    Err(_) => continue,
                                };

                                for line in result {
                                    client.send_privmsg(target, line)?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

struct Plugin {
    name: String,
    commands: Vec<String>,
}
