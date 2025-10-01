use libloading::{Library, Symbol};
use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;

#[derive(Clone)]
pub struct Plugin {
    pub name: String,
    pub commands: Vec<String>,
    pub triggers: Vec<String>,
}

const PATH: &str = "plugins/";

impl Plugin {
    pub unsafe fn load() -> Vec<Self> {
        let plugins = match fs::read_dir(PATH) {
            Ok(plugins) => plugins,
            Err(e) => {
                println!("Error loading plugins: {}", e);
                return Vec::new();
            }
        };

        let mut container = Vec::new();

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

            if !["so", "dll", "dylib"].contains(&extension) {
                continue;
            }
            println!("Loading plugin: {}", plugin.path().display());

            // Load the dynamic library
            let lib = match unsafe { Library::new(plugin.path()) } {
                Ok(lib) => lib,
                Err(e) => {
                    println!("Error loading plugin: {}", e);
                    continue;
                }
            };

            // Get a reference to the `exported` function
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

            let empty = CString::new("").unwrap().into_raw();
            // Call the `exported` function
            let raw_triggers = exported(empty, empty, empty);
            let triggers = match unsafe { CStr::from_ptr(raw_triggers).to_str() } {
                Ok(triggers) => triggers.split("\n").map(|s| s.to_string()).collect(),
                Err(_) => continue,
            };

            let raw_commands = exported(CString::new("help").unwrap().into_raw(), empty, empty);
            let commands = match unsafe { CStr::from_ptr(raw_commands).to_str() } {
                Ok(commands) => commands.split("\n").map(|s| s.to_string()).collect(),
                Err(_) => continue,
            };

            println!("Commands: {:?}", commands);

            let loaded_plugin: Plugin = Plugin {
                name: match plugin.path().to_str() {
                    Some(name) => name.to_string(),
                    None => continue,
                },
                commands: commands,
                triggers: triggers,
            };

            println!(
                "{}:\n\t{}",
                loaded_plugin.name,
                loaded_plugin.commands.join(", ")
            );

            container.push(loaded_plugin);
        }

        container
    }

    pub fn watch(
        tx_plugins: std::sync::mpsc::Sender<NotifyResult<Event>>,
    ) -> NotifyResult<RecommendedWatcher> {
        let mut watcher = notify::recommended_watcher(move |event: NotifyResult<Event>| {
            tx_plugins.send(event).unwrap();
        })?;

        watcher.watch(PATH.as_ref(), RecursiveMode::Recursive)?;

        Ok(watcher)
    }
}
