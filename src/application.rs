extern crate chrono;
extern crate reqwest;
extern crate select;

use crate::common;
use crate::plugins;

use anyhow::Result;
use futures::prelude::*;
use irc::client::prelude::*;
use libloading::{Library, Symbol};
use regex::Regex;
use std::thread;
use std::time::Duration;

pub async fn run() -> Result<(), anyhow::Error> {
    let mut loaded_plugins: Vec<plugins::Plugin> = Vec::new();

    plugins::load_plugins(&mut loaded_plugins);

    let config = Config::load("config.toml").unwrap();

    let mut client = Client::from_config(config).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    thread::spawn(|| loop {
        let now = chrono::Local::now();
        let timestamp = now.format("%T").to_string();
        println!("{}", timestamp);
        thread::sleep(Duration::from_secs(60));
    });

    while let Some(message) = stream.next().await.transpose()? {
        print!("{}", message);

        if let Command::PRIVMSG(ref _channel, ref msg) = message.command {
            if let Some(prefix) = &message.prefix {
                let author = prefix.to_string();
                let nick = author.split("!").collect::<Vec<&str>>()[0].to_string();

                if let Some(response_target) = message.response_target() {
                    let re = Regex::new(r"^([-+])([a-zA-Z\d]+)(?:\s+(.*))?$").unwrap();
                    let matched = match re.captures(msg) {
                        Some(matched) => vec![matched],
                        None => vec![],
                    };

                    if matched.len() > 0 {
                        let trigger = match matched[0].get(1) {
                            Some(s) => s.as_str(),
                            None => "",
                        };
                        let cmd = match matched[0].get(2) {
                            Some(s) => s.as_str(),
                            None => "",
                        };
                        let param = match matched[0].get(3) {
                            Some(s) => s.as_str(),
                            None => "",
                        };

                        let respond_method: fn(&Client, &str, &str) -> bool = match trigger {
                            "+" => process_privmsg,
                            "-" => process_notice,
                            _ => process_privmsg,
                        };

                        let target = match trigger {
                            "+" => response_target,
                            "-" => &nick,
                            _ => response_target,
                        };

                        // Catch commands that are handled by the bot itself
                        match cmd {
                            "help" => {
                                let mut commands: Vec<&str> = Vec::new();

                                for plugin in &loaded_plugins {
                                    for command in &plugin.commands {
                                        commands.push(command);
                                    }
                                }

                                let output = format!(
                                    "{} {}",
                                    common::l("Commands"),
                                    common::c1(&commands.join(", "))
                                );

                                respond_method(&client, target, &output);

                                continue;
                            }
                            "reload" => {
                                if author == "Dragon!~Dragon@administrator.swiftirc.net" {
                                    loaded_plugins.clear();
                                    plugins::load_plugins(&mut loaded_plugins);
                                }

                                continue;
                            }
                            _ => {}
                        }

                        // Catch commands that are handled by plugins
                        for plugin in &loaded_plugins {
                            for command in &plugin.triggers {
                                let re = match Regex::new(&command) {
                                    Ok(re) => re,
                                    Err(e) => {
                                        println!("Error loading regex: {}", e);
                                        continue;
                                    }
                                };

                                match re.captures(&cmd) {
                                    Some(_) => (),
                                    None => continue,
                                };

                                unsafe {
                                    let lib = match Library::new(plugin.name.clone()) {
                                        Ok(lib) => lib,
                                        Err(e) => {
                                            println!("Error loading plugin: {}", e);
                                            continue;
                                        }
                                    };

                                    // Load the "exported" function from the plugin
                                    let exported: Symbol<
                                        extern "C" fn(
                                            command: &str,
                                            query: &str,
                                            author: &str,
                                        )
                                            -> Result<Vec<String>, ()>,
                                    > = lib.get(b"exported\0")?;

                                    // Pass the command, query, and author to the plugin
                                    let result = match exported(cmd, param, &author) {
                                        Ok(result) => result,
                                        Err(_) => continue,
                                    };

                                    for line in result {
                                        respond_method(&client, target, &line);
                                    }
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

fn process_privmsg(client: &Client, target: &str, message: &str) -> bool {
    process_message(send_privmsg, client, target, message)
}

fn send_privmsg(client: &Client, target: &str, message: &str) -> bool {
    match client.send_privmsg(target, message) {
        Ok(_) => true,
        Err(e) => {
            println!("Error sending message: {}", e);
            false
        }
    }
}

fn process_notice(client: &Client, target: &str, message: &str) -> bool {
    process_message(send_notice, client, target, message)
}

fn send_notice(client: &Client, target: &str, message: &str) -> bool {
    match client.send_notice(target, message) {
        Ok(_) => true,
        Err(e) => {
            println!("Error sending notice: {}", e);
            false
        }
    }
}

fn process_message(
    function: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    message: &str,
) -> bool {
    let mut output: Vec<&str> = Vec::new();

    let words = message.split(" ");

    for word in words {
        output.push(word);

        if output.join(" ").len() >= 400 {
            match function(client, target, &output.join(" ")) {
                true => (),
                false => return false,
            };

            output.clear();
        }
    }

    if output.len() > 0 {
        match function(client, target, &output.join(" ")) {
            true => (),
            false => return false,
        };
    }

    true
}
