use crate::plugins::Plugin;
use common::{ColorResult, PluginContext};
use libloading::{Library, Symbol};
use log::{error, info};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct TimerDef {
    pub command: String,
    pub interval: Duration,
}

/// Parses a human-readable interval string like "1d12h", "30s", "5m" into a Duration.
/// Supported units: s (seconds), m (minutes), h (hours), d (days), w (weeks).
/// Returns None for empty, invalid, or zero-duration input.
pub fn parse_interval(input: &str) -> Option<Duration> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let mut total_secs: u64 = 0;
    let mut num_buf = String::new();
    let mut found_any = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let multiplier = match ch {
                's' => 1u64,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                _ => return None,
            };
            let n: u64 = num_buf.parse().ok().filter(|&v: &u64| v > 0 || true)?;
            num_buf.clear();
            total_secs += n * multiplier;
            found_any = true;
        }
    }

    // Trailing digits with no unit is invalid
    if !num_buf.is_empty() || !found_any {
        return None;
    }

    if total_secs == 0 {
        return None;
    }

    Some(Duration::from_secs(total_secs))
}

/// Parses a single timer declaration line like `"tracksnapshot:6h"` into a `TimerDef`.
/// Returns `None` if the line has no colon, an empty command, or an invalid interval.
pub fn parse_timer_line(line: &str) -> Option<TimerDef> {
    let (command, interval_str) = line.split_once(':')?;
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    let interval = parse_interval(interval_str)?;
    Some(TimerDef {
        command: command.to_owned(),
        interval,
    })
}

/// Parses the full output of `exported("timers")` (newline-separated lines).
/// Calls `parse_timer_line` on each non-empty line, silently skipping invalid lines.
pub fn parse_timer_declarations(output: &str) -> Vec<TimerDef> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_timer_line)
        .collect()
}

/// Spawn a tokio task for each timer declared by currently loaded plugins.
/// Each task loops: sleep(interval) then call run_timer_tick().
pub fn spawn_timers(
    active: Arc<RwLock<Vec<Plugin>>>,
    color_ffi: extern "C" fn(*const c_char, *const c_char) -> ColorResult,
    runtime: &tokio::runtime::Handle,
) -> Vec<JoinHandle<()>> {
    let plugins = match active.read() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            error!("Failed to read plugin list for timer spawning");
            return vec![];
        }
    };

    let mut handles = Vec::new();

    for plugin in &plugins {
        for timer in &plugin.timers {
            let plugin_path = plugin.name.clone();
            let command = timer.command.clone();
            let interval = timer.interval;

            info!(
                "Spawning timer '{}' for plugin '{}' every {:?}",
                command, plugin_path, interval
            );

            let handle = runtime.spawn(async move {
                loop {
                    tokio::time::sleep(interval).await;
                    run_timer_tick(&plugin_path, &command, color_ffi);
                }
            });

            handles.push(handle);
        }
    }

    handles
}

/// Execute a single timer tick: load the plugin .so, call `exported` with a
/// synthetic PluginContext, and log the result.
fn run_timer_tick(
    plugin_path: &str,
    command: &str,
    color_ffi: extern "C" fn(*const c_char, *const c_char) -> ColorResult,
) {
    let lib = match unsafe { Library::new(plugin_path) } {
        Ok(lib) => lib,
        Err(e) => {
            error!("Timer: error loading plugin '{}': {}", plugin_path, e);
            return;
        }
    };

    let results = unsafe {
        let exported: Symbol<extern "C" fn(context: &PluginContext) -> *mut c_char> =
            match lib.get(b"exported\0") {
                Ok(exported) => exported,
                Err(e) => {
                    error!("Timer: error loading 'exported' from '{}': {}", plugin_path, e);
                    return;
                }
            };

        let cstr_cmd = match CString::new(command) {
            Ok(cmd) => cmd.into_raw(),
            Err(_) => return,
        };
        let cstr_param = match CString::new("") {
            Ok(param) => param.into_raw(),
            Err(_) => return,
        };
        let cstr_author = match CString::new("timer!timer@reinze.internal") {
            Ok(author) => author.into_raw(),
            Err(_) => return,
        };

        let context = PluginContext {
            cmd: cstr_cmd,
            param: cstr_param,
            author: cstr_author,
            color: color_ffi,
        };

        let raw_results = exported(&context);

        let output = match CStr::from_ptr(raw_results).to_str() {
            Ok(results) => results
                .split("\n")
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            _ => vec![],
        };

        _ = CString::from_raw(raw_results);
        _ = CString::from_raw(cstr_author);
        _ = CString::from_raw(cstr_param);
        _ = CString::from_raw(cstr_cmd);

        output
    };

    for line in &results {
        if !line.is_empty() {
            info!("Timer [{}] {}: {}", plugin_path, command, line);
        }
    }
}


/// Manages timer lifecycle, supporting hot-reload and clean shutdown.
pub struct TimerManager {
    handles: Mutex<Vec<JoinHandle<()>>>,
    color_ffi: extern "C" fn(*const c_char, *const c_char) -> ColorResult,
    runtime_handle: Mutex<Option<tokio::runtime::Handle>>,
}

impl TimerManager {
    pub fn new(color_ffi: extern "C" fn(*const c_char, *const c_char) -> ColorResult) -> Self {
        Self {
            handles: Mutex::new(vec![]),
            color_ffi,
            runtime_handle: Mutex::new(None),
        }
    }

    /// Store the Tokio runtime handle so timers can be spawned from any thread.
    pub fn set_runtime(&self, handle: tokio::runtime::Handle) {
        *self.runtime_handle.lock().unwrap() = Some(handle);
    }

    /// Cancel all running timers and spawn new ones from the current plugin set.
    pub fn restart(&self, active: &Arc<RwLock<Vec<Plugin>>>) {
        let mut handles = self.handles.lock().unwrap();
        for handle in handles.drain(..) {
            handle.abort();
        }
        let runtime = self.runtime_handle.lock().unwrap();
        if let Some(rt) = runtime.as_ref() {
            let new_handles = spawn_timers(active.clone(), self.color_ffi, rt);
            *handles = new_handles;
        }
    }

    /// Cancel all running timers (used on disconnect).
    pub fn cancel_all(&self) {
        let mut handles = self.handles.lock().unwrap();
        for handle in handles.drain(..) {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_seconds() {
        assert_eq!(parse_interval("30s"), Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_minutes() {
        assert_eq!(parse_interval("5m"), Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_hours() {
        assert_eq!(parse_interval("6h"), Some(Duration::from_secs(21600)));
    }

    #[test]
    fn test_days() {
        assert_eq!(parse_interval("1d"), Some(Duration::from_secs(86400)));
    }

    #[test]
    fn test_weeks() {
        assert_eq!(parse_interval("1w"), Some(Duration::from_secs(604800)));
    }

    #[test]
    fn test_combined_units() {
        assert_eq!(
            parse_interval("1d12h"),
            Some(Duration::from_secs(86400 + 43200))
        );
    }

    #[test]
    fn test_all_units() {
        // 1w + 2d + 3h + 4m + 5s = 604800 + 172800 + 10800 + 240 + 5 = 788645
        assert_eq!(
            parse_interval("1w2d3h4m5s"),
            Some(Duration::from_secs(788645))
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(parse_interval(""), None);
    }

    #[test]
    fn test_invalid_input() {
        assert_eq!(parse_interval("abc"), None);
        assert_eq!(parse_interval("10x"), None);
        assert_eq!(parse_interval("hello"), None);
    }

    #[test]
    fn test_zero() {
        assert_eq!(parse_interval("0s"), None);
        assert_eq!(parse_interval("0m"), None);
        assert_eq!(parse_interval("0h0m0s"), None);
    }

    #[test]
    fn test_parse_timer_line() {
        let def = parse_timer_line("tracksnapshot:6h");
        assert!(def.is_some());
        let def = def.unwrap();
        assert_eq!(def.command, "tracksnapshot");
        assert_eq!(def.interval, Duration::from_secs(21600));
    }

    #[test]
    fn test_parse_timer_line_invalid_no_colon() {
        assert!(parse_timer_line("tracksnapshot").is_none());
    }

    #[test]
    fn test_parse_timer_line_invalid_interval() {
        assert!(parse_timer_line("tracksnapshot:abc").is_none());
    }

    #[test]
    fn test_parse_timer_line_empty_command() {
        assert!(parse_timer_line(":6h").is_none());
    }

    #[test]
    fn test_parse_timer_declarations() {
        let input = "tracksnapshot:6h\ncleanup:1d";
        let defs = parse_timer_declarations(input);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].command, "tracksnapshot");
        assert_eq!(defs[1].command, "cleanup");
        assert_eq!(defs[1].interval, Duration::from_secs(86400));
    }

    #[test]
    fn test_parse_timer_declarations_empty() {
        let defs = parse_timer_declarations("");
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn test_parse_timer_declarations_skips_invalid() {
        let input = "good:1h\nbad\nalso_good:30m";
        let defs = parse_timer_declarations(input);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].command, "good");
        assert_eq!(defs[1].command, "also_good");
    }
}
