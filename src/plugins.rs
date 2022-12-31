use libloading::{Library, Symbol};
use std::fs;

pub struct Plugin {
    pub name: String,
    pub commands: Vec<String>,
    pub triggers: Vec<String>,
}

pub fn load_plugins(loaded_plugins: &mut Vec<Plugin>) {
    let plugins = match fs::read_dir("plugins/") {
        Ok(plugins) => plugins,
        Err(e) => {
            println!("Error loading plugins: {}", e);
            return;
        }
    };

    for plugin in plugins {
        let plugin = match plugin {
            Ok(plugin) => plugin,
            Err(e) => {
                println!("Error loading plugins: {}", e);
                continue;
            }
        };

        let path = plugin.path();
        let extension = match path.extension() {
            Some(ext) => match ext.to_str() {
                Some(ext) => ext,
                None => continue,
            },
            None => continue,
        };

        if ["so", "dll"].contains(&extension) {
            println!("Loading plugin: {}", plugin.path().display());

            unsafe {
                // Load the dynamic library
                let lib = match Library::new(plugin.path()) {
                    Ok(lib) => lib,
                    Err(e) => {
                        println!("Error loading plugin: {}", e);
                        continue;
                    }
                };

                // Get a reference to the `exported` function
                let exported: Symbol<
                    extern "C" fn(
                        command: &str,
                        query: &str,
                        author: &str,
                    ) -> Result<Vec<String>, ()>,
                > = match lib.get(b"exported\0") {
                    Ok(exported) => exported,
                    Err(e) => {
                        println!("Error loading plugin: {}", e);
                        continue;
                    }
                };

                // Call the `exported` function
                let functions = match exported("", "", "") {
                    Ok(functions) => functions,
                    Err(_) => continue,
                };

                let commands = match exported("help", "", "") {
                    Ok(commands) => commands,
                    Err(_) => continue,
                };

                println!("Commands: {:?}", commands);

                let loaded_plugin: Plugin = Plugin {
                    name: match plugin.path().to_str() {
                        Some(name) => name.to_string(),
                        None => continue,
                    },
                    commands: commands,
                    triggers: functions,
                };

                loaded_plugins.push(loaded_plugin);
            }
        }
    }

    // Print out valid commands at startup
    for plugin in loaded_plugins {
        println!(".Plugin: {}", plugin.name);
        for command in &plugin.commands {
            println!("..Command: {}", command);
        }
    }
}
