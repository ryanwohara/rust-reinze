extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use crate::plugins::{Plugin, PluginManager};
use common::author::Author;
use futures::prelude::*;
use irc::client::prelude::*;
use libloading::{Library, Symbol};
use regex::Regex;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::{task, time};

pub async fn run<T>(path: T)
where
    T: ToString,
{
    let config = Config::load(path.to_string()).unwrap();
    let mut interval = 1;

    loop {
        let plugin_manager = PluginManager::new();
        plugin_manager.reload().unwrap();

        let active_ref = plugin_manager.active.clone();

        thread::spawn(move || plugin_manager.watch());

        let before = time::Instant::now();
        run_client(&config, active_ref).await;
        let after = time::Instant::now();
        let difference = after - before;
        interval = if difference.as_secs() > 300 {
            1
        } else {
            interval
        };

        eprintln!(
            "Disconnected. Waiting {} secs before trying again...",
            interval
        );

        time::sleep(Duration::from_secs(interval)).await;
        interval = 2 * interval;
    }
}

async fn run_client(config: &Config, active: Arc<RwLock<Vec<Plugin>>>) {
    let mut client = Client::from_config(config.to_owned()).await.unwrap();
    client.identify().unwrap();
    let mut stream = client.stream().unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let plugins = match active.read() {
                Ok(g) => g.clone(),
                _ => continue,
            };

            if !handle_incoming_message(&client, &message, plugins).await {
                eprintln!("Error handling message: {}", message.to_string());
            }
        }
    });

    while let Ok(Some(message)) = stream.next().await.transpose() {
        print!(
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            message
        );

        tx.send(message).ok();
    }
}

async fn handle_incoming_message(
    client: &Client,
    message: &Message,
    loaded_plugins: Vec<Plugin>,
) -> bool {
    let ref msg = match message.command {
        Command::PRIVMSG(ref _channel, ref msg) => msg,
        Command::NOTICE(ref _channel, ref msg) => msg,
        _ => return true,
    };

    let prefix = match message.prefix {
        Some(ref prefix) => prefix,
        None => return true,
    };

    let author = prefix.to_string();
    let nick: String = author.split("!").collect::<Vec<&str>>()[0].to_string();

    let response_target = match message.response_target() {
        Some(target) => target,
        None => return true,
    };

    let re = Regex::new(r"^([-+])([a-zA-Z\d-]+)(?:\s+(.*))?$").unwrap();
    let matched = match re.captures(msg) {
        Some(matched) => vec![matched],
        None => vec![],
    };

    // if the regex match fails, just return
    if matched.is_empty() {
        return true;
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
        Some(s) => s.as_str().trim(),
        None => "",
    };

    let respond_method: fn(&Client, &str, &str) -> bool = match trigger {
        "-" => process_notice,
        "+" | _ => process_privmsg,
    };

    let target = match trigger {
        "-" => &nick,
        "+" | _ => response_target,
    };

    // Catch commands that are handled by the bot itself
    handle_core_messages(
        respond_method,
        client,
        target,
        &loaded_plugins,
        &author,
        cmd,
    )
    .await
        || handle_plugin_messages(
            respond_method,
            client,
            target,
            &loaded_plugins,
            &author,
            cmd,
            param,
        )
        .await
}

async fn handle_core_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &Vec<Plugin>,
    a: &str,
    cmd: &str,
) -> bool {
    let author = Author::create(a);

    match cmd {
        "help" => {
            let commands = loaded_plugins
                .iter()
                .map(|plugin| plugin.commands.to_owned())
                .flatten()
                .collect::<Vec<String>>();

            let output = task::spawn_blocking(move || {
                vec![author.l("Commands"), author.c1(&commands.join(", "))].join(" ")
            })
            .await
            .unwrap();

            respond_method(&client, target, &output);

            true
        }
        _ => false,
    }
}

async fn handle_plugin_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &Vec<Plugin>,
    author: &str,
    cmd: &str,
    param: &str,
) -> bool {
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
                if line.is_empty() {
                    continue;
                }
                respond_method(&client, target, &line);
            }
        }
    }

    true
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

    let words = message.split_whitespace();
    let flush = |out: &mut Vec<&str>| {
        let joined = out.join(" ");
        let ok = function(client, target, &joined);
        out.clear();
        ok
    };

    for word in words {
        output.push(word);

        if output.join(" ").len() >= 400 && !flush(&mut output) {
            return false;
        }
    }

    !output.is_empty() && flush(&mut output)
}
