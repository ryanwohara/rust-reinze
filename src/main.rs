pub mod common;
extern crate reqwest;
extern crate select;
mod plugins;
use crate::common::c1;
use crate::common::l;
use anyhow::Result;
use futures::prelude::*;
use irc::client::prelude::*;
use libloading::{Library, Symbol};
use regex::Regex;
use tokio;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut loaded_plugins: Vec<plugins::Plugin> = Vec::new();

    plugins::load_plugins(&mut loaded_plugins);

    // Print out valid commands at startup
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

                    // Catch commands that are handled by the bot itself
                    match cmd {
                        "help" => {
                            let mut commands: Vec<&str> = Vec::new();

                            for plugin in &loaded_plugins {
                                for command in &plugin.commands {
                                    commands.push(command);
                                }
                            }

                            let output = format!("{} {}", l("Commands"), c1(&commands.join(", ")));

                            send_privmsg(&client, target, &output);
                        }
                        _ => {}
                    }

                    // Catch commands that are handled by plugins
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
                                    send_privmsg(&client, target, &line);
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

fn send_privmsg(client: &Client, target: &str, message: &str) -> bool {
    let mut output: Vec<&str> = Vec::new();

    let words = message.split(" ");

    for word in words {
        output.push(word);

        if output.join(" ").len() >= 400 {
            match client.send_privmsg(target, output.join(" ")) {
                Ok(_) => (),
                Err(e) => {
                    println!("Error sending message: {}", e);
                    return false;
                }
            };
            output.clear();
        }
    }

    if output.len() > 0 {
        match client.send_privmsg(target, output.join(" ")) {
            Ok(_) => (),
            Err(e) => {
                println!("Error sending message: {}", e);

                return false;
            }
        };
    }

    return true;
}
