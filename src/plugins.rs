use libloading::{Library, Symbol};
use std::fs;

pub struct Plugin {
    pub name: String,
    pub commands: Vec<String>,
}

pub fn load_plugins(loaded_plugins: &mut Vec<Plugin>) {
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
                let lib = Library::new(plugin.path()).unwrap();

                // Get a reference to the `exported` function
                let exported: Symbol<
                    extern "C" fn(command: &str, query: &str) -> Result<Vec<String>, ()>,
                > = lib.get(b"exported\0").unwrap();

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

    // Print out valid commands at startup
    for plugin in loaded_plugins {
        println!(".Plugin: {}", plugin.name);
        for command in &plugin.commands {
            println!("..Command: {}", command);
        }
    }
}
