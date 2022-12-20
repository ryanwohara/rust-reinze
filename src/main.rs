pub mod common;
pub mod runescape;

extern crate reqwest;
extern crate select;

use crate::common::c1;
use crate::common::l;
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

        if plugin.path().extension().unwrap() == "so" {
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

    for plugin in loaded_plugins {
        println!(".Plugin: {}", plugin.name);
        for command in plugin.commands {
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

                    match cmd {
                        "ping" => {
                            match client.send_privmsg(target, "pong!") {
                                Ok(_) => {}
                                Err(e) => {
                                    println!("Error sending message: {}", e);
                                }
                            };
                        }
                        "players" => {
                            match runescape::players().await {
                                Ok(message) => {
                                    match client.send_privmsg(target, message) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error sending message: {}", e);
                                        }
                                    };
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting player count")?;
                                }
                            };
                        }
                        "params" => {
                            let params: (&str, &str) = match param.split_once(" ") {
                                Some(params) => params,
                                None => {
                                    client.send_privmsg(target, "Invalid number of arguments")?;
                                    continue;
                                }
                            };

                            if params.0.is_empty() || params.1.is_empty() {
                                client.send_privmsg(target, "Invalid number of arguments")?;
                                continue;
                            }

                            match runescape::params(params.0, params.1).await {
                                Ok(message) => {
                                    match client.send_privmsg(target, message) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error sending message: {}", e);
                                        }
                                    };
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting params")?;
                                }
                            };
                        }
                        "price" => {
                            if param.is_empty() {
                                client.send_privmsg(target, "Invalid number of arguments")?;
                                continue;
                            }

                            match runescape::prices(param).await {
                                Ok(message) => {
                                    match client.send_privmsg(target, message) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error sending message: {}", e);
                                        }
                                    };
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting price")?;
                                }
                            };
                        }
                        "ge" => {
                            if param.is_empty() {
                                client.send_privmsg(target, "Invalid number of arguments")?;
                                continue;
                            }

                            match runescape::ge(param).await {
                                Ok(message) => {
                                    match client.send_privmsg(target, message) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error sending message: {}", e);
                                        }
                                    };
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting price")?;
                                }
                            };
                        }
                        "boss" => {
                            if param.is_empty() {
                                client.send_privmsg(target, "Invalid number of arguments")?;
                                continue;
                            }

                            match runescape::boss(param).await {
                                Ok(boss_kills) => {
                                    // let output = format!("{} {}", l("Boss"), boss_kills.join(&c1(" | ")));

                                    let prefix = l("Boss");
                                    let mut output_boss_kills: Vec<String> = Vec::new();

                                    let mut output;

                                    for boss in boss_kills {
                                        output_boss_kills.push(boss);

                                        output = format!(
                                            "{} {}",
                                            prefix,
                                            output_boss_kills.join(&c1(" | "))
                                        );

                                        if output_boss_kills.len() >= 8 {
                                            match client.send_privmsg(target, output) {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    println!("Error sending message: {}", e);
                                                }
                                            };

                                            output_boss_kills.clear();
                                        }
                                    }

                                    if output_boss_kills.len() > 0 {
                                        output = format!(
                                            "{} {}",
                                            prefix,
                                            output_boss_kills.join(&c1(" | "))
                                        );
                                        match client.send_privmsg(target, output) {
                                            Ok(_) => {}
                                            Err(e) => {
                                                println!("Error sending message: {}", e);
                                            }
                                        };
                                    }
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting price")?;
                                }
                            };
                        }
                        _ => {}
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
