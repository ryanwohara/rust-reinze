extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use crate::plugins::{Plugin, PluginManager};
use anyhow::Result;
use futures::prelude::*;
use irc::client::prelude::*;
use libloading::{Library, Symbol};
use regex::Regex;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio::sync::mpsc;
use tokio::task;

pub async fn run(path: &str) {
    let config = Config::load(path).unwrap();

    let plugin_manager = PluginManager::new();
    plugin_manager.reload().unwrap();

    let active_ref = plugin_manager.active.clone();

    thread::spawn(move || plugin_manager.watch());

    run_client(&config, active_ref)
        .await
        .expect("Critical failure");
}

async fn run_client(
    config: &Config,
    active: Arc<RwLock<Vec<Plugin>>>,
) -> Result<(), anyhow::Error> {
    let mut client = Client::from_config(config.to_owned()).await?;
    client.identify()?;
    let mut stream = client.stream()?;

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let plugins = match active.read() {
                Ok(g) => g.clone(),
                _ => continue,
            };

            if let Err(e) = handle_incoming_message(&client, message, plugins).await {
                eprintln!("Error handling message: {}", e);
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        print!(
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            message
        );

        tx.send(message).ok();
    }

    Ok(())
}

async fn handle_incoming_message(
    client: &Client,
    message: Message,
    loaded_plugins: Vec<Plugin>,
) -> Result<(), anyhow::Error> {
    let ref msg = match message.command {
        Command::PRIVMSG(ref _channel, ref msg) => msg,
        Command::NOTICE(ref _channel, ref msg) => msg,
        _ => return Ok(()),
    };

    let prefix = match message.prefix {
        Some(ref prefix) => prefix,
        None => return Ok(()),
    };

    let author = prefix.to_string();
    let nick: String = author.split("!").collect::<Vec<&str>>()[0].to_string();

    let response_target = match message.response_target() {
        Some(target) => target,
        None => return Ok(()),
    };

    let re = Regex::new(r"^([-+])([a-zA-Z\d-]+)(?:\s+(.*))?$")?;
    let matched = match re.captures(msg) {
        Some(matched) => vec![matched],
        None => vec![],
    };

    // if the regex match fails, just return
    if matched.is_empty() {
        return Ok(());
    }
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
    match handle_core_messages(
        respond_method,
        client,
        target,
        &loaded_plugins,
        &author,
        cmd,
    )
    .await
    {
        true => return Ok(()),
        false => (),
    };

    handle_plugin_messages(
        respond_method,
        client,
        target,
        &loaded_plugins,
        &author,
        cmd,
        param,
    )
    .await;

    Ok(())
}

async fn handle_core_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &Vec<Plugin>,
    _author: &str,
    cmd: &str,
) -> bool {
    match cmd {
        "help" => {
            let mut commands: Vec<&str> = Vec::new();

            for plugin in loaded_plugins {
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

            return true;
        }
        _ => {}
    }

    false
}

async fn handle_plugin_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &Vec<Plugin>,
    author: &str,
    cmd: &str,
    param: &str,
) {
    // Catch commands that are handled by plugins
    for plugin in loaded_plugins {
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

            let lib = match unsafe { Library::new(plugin.name.clone()) } {
                Ok(lib) => lib,
                Err(e) => {
                    println!("Error loading plugin: {}", e);
                    continue;
                }
            };

            let author = author.to_string();
            let cmd = cmd.to_string();
            let param = param.to_string();

            // Pass the command, query, and author to the plugin
            let results = match task::spawn_blocking(move || unsafe {
                // Load the "exported" function from the plugin
                let exported: Symbol<
                    extern "C" fn(
                        command: *const c_char,
                        query: *const c_char,
                        author: *const c_char,
                    ) -> *mut c_char,
                > = match lib.get(b"exported\0") {
                    Ok(exported) => exported,
                    Err(e) => {
                        println!("Error loading plugin: {}", e);
                        return vec!["".to_string()];
                    }
                };
                // Convert the command, query, and author to C strings
                let cstr_cmd = match CString::new(cmd) {
                    Ok(cmd) => cmd.into_raw(),
                    Err(_) => return vec!["".to_string()],
                };
                let cstr_param = match CString::new(param) {
                    Ok(param) => param.into_raw(),
                    Err(_) => return vec!["".to_string()],
                };
                let cstr_author = match CString::new(author.to_owned()) {
                    Ok(author) => author.into_raw(),
                    Err(_) => return vec!["".to_string()],
                };

                let raw_results = exported(cstr_cmd, cstr_param, cstr_author);

                match CStr::from_ptr(raw_results).to_str() {
                    Ok(results) => results
                        .split("\n")
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                    _ => return vec!["".to_string()],
                }
            })
            .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };

            for line in results {
                if line.len() == 0 {
                    continue;
                }
                respond_method(&client, target, &line);
            }
        }
    }
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
