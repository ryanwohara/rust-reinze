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
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::thread;
use std::time::Duration;

pub async fn run() {
    let mut loaded_plugins: Vec<plugins::Plugin> = Vec::new();

    plugins::load_plugins(&mut loaded_plugins);

    let config = Config::load("config.toml").unwrap();

    let mut iterations = 0;

    loop {
        match run_client(&config, &mut loaded_plugins).await {
            Ok(_) => {
                iterations = 0;
                continue;
            }
            Err(e) => println!("Error running client: {}", e),
        };
        iterations += 1;
        println!(
            "Restarting in {} seconds (iterations: {})",
            5 * iterations,
            iterations
        );
        thread::sleep(Duration::from_secs(5 * iterations));
    }
}

async fn run_client(
    config: &Config,
    loaded_plugins: &mut Vec<plugins::Plugin>,
) -> Result<(), anyhow::Error> {
    let mut client = Client::from_config(config.to_owned()).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    thread::spawn(|| loop {
        let now = chrono::Local::now();
        let timestamp = now.format("%T").to_string();
        println!("{}", timestamp);
        thread::sleep(Duration::from_secs(60));
    });

    loop {
        let message = match stream.next().await {
            Some(Ok(message)) => message,
            Some(Err(e)) => {
                println!("Error: {}", e);
                return Ok(());
            }
            None => {
                println!("Stream closed");
                return Ok(());
            }
        };

        print!("{}", message);

        match handle_incoming_message(&client, message, loaded_plugins) {
            Ok(_) => continue,
            Err(e) => {
                println!("Error handling message: {}", e);
                break;
            }
        };
    }

    Ok(())
}

fn handle_incoming_message(
    client: &Client,
    message: Message,
    loaded_plugins: &mut Vec<plugins::Plugin>,
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

    let re = Regex::new(r"^([-+])([a-zA-Z\d]+)(?:\s+(.*))?$").unwrap();
    let matched = match re.captures(msg) {
        Some(matched) => vec![matched],
        None => vec![],
    };

    // if the regex match fails, just return
    if matched.len() == 0 {
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
    match handle_core_messages(respond_method, client, target, loaded_plugins, &author, cmd) {
        true => return Ok(()),
        false => (),
    };

    handle_plugin_messages(
        respond_method,
        client,
        target,
        loaded_plugins,
        &author,
        cmd,
        param,
    );

    Ok(())
}

fn handle_core_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &mut Vec<plugins::Plugin>,
    author: &str,
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
        "reload" => {
            if author == "Dragon!~Dragon@administrator.swiftirc.net" {
                loaded_plugins.clear();
                plugins::load_plugins(loaded_plugins);
            }

            return true;
        }
        _ => {}
    }

    false
}

fn handle_plugin_messages(
    respond_method: fn(&Client, &str, &str) -> bool,
    client: &Client,
    target: &str,
    loaded_plugins: &mut Vec<plugins::Plugin>,
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

            // Load the "exported" function from the plugin
            let exported: Symbol<
                extern "C" fn(
                    command: *const c_char,
                    query: *const c_char,
                    author: *const c_char,
                ) -> *mut c_char,
            > = match unsafe { lib.get(b"exported\0") } {
                Ok(exported) => exported,
                Err(e) => {
                    println!("Error loading plugin: {}", e);
                    continue;
                }
            };

            // Convert the command, query, and author to C strings
            let cstr_cmd = match CString::new(cmd) {
                Ok(cmd) => cmd.into_raw(),
                Err(_) => continue,
            };
            let cstr_param = match CString::new(param) {
                Ok(param) => param.into_raw(),
                Err(_) => continue,
            };
            let cstr_author = match CString::new(author.to_owned()) {
                Ok(author) => author.into_raw(),
                Err(_) => continue,
            };

            // Pass the command, query, and author to the plugin
            let raw_result = exported(cstr_cmd, cstr_param, cstr_author);
            let results = match unsafe { CStr::from_ptr(raw_result).to_str() } {
                Ok(result) => result.split("\n").map(|s| s.to_string()),
                Err(_) => continue,
            };

            for line in results {
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
