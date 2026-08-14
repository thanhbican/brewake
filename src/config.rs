use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{self, ErrorKind},
    path::PathBuf,
};

pub const APP_DIR: &str = "Brewake";
pub const CONFIG_FILE: &str = "config.json";
pub const RUNTIME_FILE: &str = "runtime.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// Whether Brewake begins keeping the computer awake when it launches.
    pub enabled: bool,
    /// Whether an active session also prevents the display from sleeping.
    pub keep_display_on: bool,
    /// Mirrors the per-user Windows startup registry entry.
    pub launch_at_startup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            keep_display_on: false,
            launch_at_startup: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatus {
    pub running: bool,
    pub enabled: bool,
    pub keep_display_on: bool,
    /// Whether the most recent SetThreadExecutionState call was accepted by Windows.
    #[serde(default)]
    pub power_request_succeeded: bool,
    pub mode: String,
    pub remaining_seconds: Option<u64>,
    /// Absolute deadline lets the CLI show a live countdown without disk writes every second.
    pub deadline_unix_seconds: Option<u64>,
}

impl RuntimeStatus {
    pub fn stopped() -> Self {
        Self {
            running: false,
            enabled: false,
            keep_display_on: false,
            power_request_succeeded: false,
            mode: "stopped".into(),
            remaining_seconds: None,
            deadline_unix_seconds: None,
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join(APP_DIR)
}

pub fn config_path() -> PathBuf {
    app_data_dir().join(CONFIG_FILE)
}

pub fn runtime_path() -> PathBuf {
    app_data_dir().join(RUNTIME_FILE)
}

pub fn load_config() -> Config {
    read_json(config_path()).unwrap_or_default()
}

pub fn save_config(config: &Config) -> io::Result<()> {
    write_json(config_path(), config)
}

pub fn load_runtime() -> RuntimeStatus {
    read_json(runtime_path()).unwrap_or_else(RuntimeStatus::stopped)
}

pub fn save_runtime(status: &RuntimeStatus) -> io::Result<()> {
    write_json(runtime_path(), status)
}

pub fn clear_runtime() -> io::Result<()> {
    match fs::remove_file(runtime_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Option<T> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(value)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_receive_safe_defaults() {
        let config: Config = serde_json::from_str(r#"{"enabled":false}"#).unwrap();
        assert_eq!(
            config,
            Config {
                enabled: false,
                keep_display_on: false,
                launch_at_startup: false,
            }
        );
    }
}
