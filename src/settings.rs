//! Small JSON settings store shared by the chrome bridge and the settings page.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub home: String,
    pub cookies: bool,
    pub dnt: bool,
    pub compact_tabs: bool,
    pub show_status: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            home: "https://yahoo.com/".to_string(),
            cookies: true,
            dnt: false,
            compact_tabs: false,
            show_status: true,
        }
    }
}

impl Settings {
    /// Accept the object sent by the settings page. Unknown keys are ignored;
    /// an empty home page is replaced with the default.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let mut settings: Settings = serde_json::from_value(value.clone()).ok()?;
        if settings.home.trim().is_empty() {
            settings.home = Settings::default().home;
        }
        Some(settings)
    }
}

pub fn config_dir() -> PathBuf {
    default_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir)
}

pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("FDB_SETTINGS_PATH") {
        return PathBuf::from(path);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library/Application Support/Free Domain Browser CEF/settings.json");
    }
    std::env::temp_dir().join("free-domain-browser-cef-settings.json")
}

pub fn load_from(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, json)
}

fn fallback_path() -> PathBuf {
    std::env::temp_dir().join("free-domain-browser-cef-settings.json")
}

pub fn load() -> Settings {
    let primary = default_path();
    if primary.exists() {
        return load_from(&primary);
    }
    load_from(&fallback_path())
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    // The app may be sandboxed without Application Support access (for example
    // during `cargo run`); keep a temp fallback so settings still persist.
    save_to(&default_path(), settings).or_else(|_| save_to(&fallback_path(), settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_settings() {
        let dir = std::env::temp_dir().join(format!("fdb-settings-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let settings = Settings {
            home: "https://example.com/".into(),
            cookies: false,
            dnt: true,
            compact_tabs: true,
            show_status: false,
        };
        save_to(&path, &settings).unwrap();
        assert_eq!(load_from(&path), settings);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_home_falls_back_to_default() {
        let value = serde_json::json!({"home":"  ","cookies":false,"dnt":true});
        assert_eq!(Settings::from_value(&value).unwrap().home, Settings::default().home);
    }
}
