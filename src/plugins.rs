use crate::timers::{TimerDef, TimerManager, parse_timer_declarations};
use common::author::cache::color_ffi;
use common::{ColorResult, PluginContext};
use libloading::{Library, Symbol};
use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct PluginManager {
    pub active: Arc<RwLock<Vec<Plugin>>>,
    pub grave: Arc<Mutex<Vec<Plugin>>>,
    pub timer_manager: Arc<TimerManager>,
}

impl PluginManager {
    pub fn new(color_ffi: extern "C" fn(*const c_char, *const c_char) -> ColorResult) -> Self {
        Self {
            active: Arc::new(RwLock::new(Vec::new())),
            grave: Arc::new(Mutex::new(Vec::new())),
            timer_manager: Arc::new(TimerManager::new(color_ffi)),
        }
    }

    pub fn add(&self, path: &str) {
        println!("... Adding plugin {}", path);
        // Load the dynamic library
        let lib = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                println!("Error loading plugin: {}", e);
                return;
            }
        };

        // Get a reference to the `exported` function
        let exported: Symbol<extern "C" fn(context: &PluginContext) -> *mut c_char> =
            match unsafe { lib.get(b"exported\0") } {
                Ok(exported) => exported,
                Err(e) => {
                    println!("Error loading plugin: {}", e);
                    return;
                }
            };

        let empty = CString::new("").unwrap().into_raw();
        // Call the `exported` function
        let raw_triggers = exported(&PluginContext {
            cmd: empty,
            param: empty,
            author: empty,
            color: color_ffi,
        });

        let triggers = match unsafe { CStr::from_ptr(raw_triggers).to_str() } {
            Ok(triggers) => triggers.split("\n").map(|s| s.to_string()).collect(),
            Err(_) => return,
        };

        let raw_commands = exported(&PluginContext {
            cmd: CString::new("help").unwrap().into_raw(),
            param: empty,
            author: empty,
            color: color_ffi,
        });
        let commands = match unsafe { CStr::from_ptr(raw_commands).to_str() } {
            Ok(commands) => commands.split("\n").map(|s| s.to_string()).collect(),
            Err(_) => return,
        };

        let raw_timers = exported(&PluginContext {
            cmd: CString::new("timers").unwrap().into_raw(),
            param: empty,
            author: empty,
            color: color_ffi,
        });
        let timers = match unsafe { CStr::from_ptr(raw_timers).to_str() } {
            Ok(timers_str) => parse_timer_declarations(timers_str),
            Err(_) => vec![],
        };

        let plugin: Plugin = Plugin {
            name: path.to_string(),
            commands,
            triggers,
            timers,
        };

        self.active.write().unwrap().push(plugin);
        self.timer_manager.restart(&self.active);
    }

    pub fn reload(&self) -> Result<&Self, ()> {
        let mut active_ref = match self.active.write() {
            Ok(guard) => guard,
            Err(_) => return Err(()),
        };

        let mut grave_ref = match self.grave.lock() {
            Ok(guard) => guard,
            Err(_) => return Err(()),
        };

        let plugins = match fs::read_dir(PATH) {
            Ok(plugins) => plugins,
            Err(e) => {
                println!("Error loading plugins: {}", e);
                return Err(());
            }
        };

        let mut new = Vec::new();

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

            // Load the dynamic library
            let lib = match unsafe { Library::new(plugin.path()) } {
                Ok(lib) => lib,
                Err(e) => {
                    println!("Error loading plugin: {}", e);
                    continue;
                }
            };

            // Get a reference to the `exported` function
            let exported: Symbol<extern "C" fn(context: &PluginContext) -> *mut c_char> =
                match unsafe { lib.get(b"exported\0") } {
                    Ok(exported) => exported,
                    Err(e) => {
                        println!("Error loading plugin: {}", e);
                        continue;
                    }
                };

            let empty = CString::new("").unwrap().into_raw();
            // Call the `exported` function
            let raw_triggers = exported(&PluginContext {
                cmd: empty,
                param: empty,
                author: empty,
                color: color_ffi,
            });
            let triggers = match unsafe { CStr::from_ptr(raw_triggers).to_str() } {
                Ok(triggers) => triggers.split("\n").map(|s| s.to_string()).collect(),
                Err(_) => continue,
            };

            let raw_commands = exported(&PluginContext {
                cmd: CString::new("help").unwrap().into_raw(),
                param: empty,
                author: empty,
                color: color_ffi,
            });

            let commands = match unsafe { CStr::from_ptr(raw_commands).to_str() } {
                Ok(commands) => commands.split("\n").map(|s| s.to_string()).collect(),
                Err(_) => continue,
            };

            let raw_timers = exported(&PluginContext {
                cmd: CString::new("timers").unwrap().into_raw(),
                param: empty,
                author: empty,
                color: color_ffi,
            });
            let timers = match unsafe { CStr::from_ptr(raw_timers).to_str() } {
                Ok(timers_str) => parse_timer_declarations(timers_str),
                Err(_) => vec![],
            };

            let name = match plugin.path().to_str() {
                Some(name) => name.to_string(),
                None => continue,
            };

            let loaded_plugin: Plugin = Plugin {
                name,
                commands,
                triggers,
                timers,
            };

            println!(
                "{}:\n\t{}",
                loaded_plugin.name,
                loaded_plugin.commands.join(", ")
            );

            if !loaded_plugin.timers.is_empty() {
                println!(
                    "\tTimers: {}",
                    loaded_plugin
                        .timers
                        .iter()
                        .map(|t| format!("{}:{:?}", t.command, t.interval))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            new.push(loaded_plugin);
        }

        let old = std::mem::replace(&mut *active_ref, new);

        grave_ref.extend(old);

        // Drop locks before restarting timers — restart() needs to read-lock active
        drop(active_ref);
        drop(grave_ref);

        self.timer_manager.restart(&self.active);

        Ok(self)
    }

    pub fn watch(&self) {
        let (tx, rx) = channel();
        println!("Watching plugin changes...");
        let _watcher = Plugin::watch(tx);

        loop {
            let event = rx.recv();

            match event {
                Ok(Ok(e)) if e.kind.is_remove() || e.kind.is_modify() => {
                    let path = e.paths.first().unwrap().to_string_lossy().to_string();

                    println!(
                        "Plugin change detected! {} {}",
                        if e.kind.is_remove() {
                            "Removed"
                        } else {
                            "Created"
                        },
                        path
                    );

                    if e.kind.is_remove() {
                        self.reload().expect("Plugin loading error");
                    } else {
                        self.add(&path);
                    }
                }
                Ok(Err(_)) => continue,
                _ => continue,
            }
        }
    }
}

#[derive(Clone)]
pub struct Plugin {
    pub name: String,
    pub commands: Vec<String>,
    pub triggers: Vec<String>,
    pub timers: Vec<TimerDef>,
}

const PATH: &str = "plugins/";

impl Plugin {
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
