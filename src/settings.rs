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
    pub density: String,
    pub theme: String,
    pub skin: String,
    pub custom_css: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // An empty home page means the start page, the same way the home
            // button works. It must not be a search engine: the browser opens
            // on its own start page until the user picks a page of their own.
            home: String::new(),
            cookies: true,
            dnt: false,
            compact_tabs: false,
            density: "comfortable".to_string(),
            theme: "system".to_string(),
            skin: "default".to_string(),
            custom_css: String::new(),
        }
    }
}

impl Settings {
    /// Accept the object sent by the settings page. Unknown keys are ignored.
    /// An empty home page is kept: it means the start page.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let mut settings: Settings = serde_json::from_value(value.clone()).ok()?;
        if !matches!(settings.theme.as_str(), "system" | "light" | "dark") {
            settings.theme = Settings::default().theme;
        }
        if !matches!(settings.density.as_str(), "comfortable" | "compact") {
            settings.density = Settings::default().density;
        }
        if !matches!(
            settings.skin.as_str(),
            "default"
                | "high-tech"
                | "futuristic"
                | "historic"
                | "victorian"
                | "amazon"
                | "terminal"
        ) {
            settings.skin = Settings::default().skin;
        }
        if settings.custom_css.len() > 24_000 {
            let mut end = 24_000;
            while !settings.custom_css.is_char_boundary(end) {
                end -= 1;
            }
            settings.custom_css.truncate(end);
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
    // An explicit settings path is an isolation boundary (tests and parallel
    // profiles). Do not leak the shared fallback file into that profile.
    if std::env::var_os("FDB_SETTINGS_PATH").is_some() {
        return Settings::default();
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
            theme: "dark".into(),
            density: "compact".into(),
            skin: "terminal".into(),
            custom_css: "body{background:#000}".into(),
        };
        save_to(&path, &settings).unwrap();
        assert_eq!(load_from(&path), settings);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn explicit_path_does_not_read_the_shared_fallback() {
        let dir =
            std::env::temp_dir().join(format!("fdb-settings-explicit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        // SAFETY: environment mutation is process-global. This test owns the
        // variable for its duration and restores it before returning.
        let previous = std::env::var_os("FDB_SETTINGS_PATH");
        unsafe {
            std::env::set_var("FDB_SETTINGS_PATH", &path);
        }
        let settings = load();
        unsafe {
            match previous {
                Some(value) => std::env::set_var("FDB_SETTINGS_PATH", value),
                None => std::env::remove_var("FDB_SETTINGS_PATH"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn custom_css_truncates_on_a_char_boundary() {
        let css = format!("{}é", "a".repeat(23_999));
        let value = serde_json::json!({"custom_css": css});
        let settings = Settings::from_value(&value).unwrap();
        assert!(settings.custom_css.len() <= 24_000);
        assert!(
            settings
                .custom_css
                .is_char_boundary(settings.custom_css.len())
        );
    }

    #[test]
    fn an_empty_home_page_is_kept() {
        // An empty home page means the start page. It must not fall back to
        // a page the user never asked for.
        let value = serde_json::json!({"home":"  ","cookies":false,"dnt":true});
        assert_eq!(Settings::from_value(&value).unwrap().home, "  ");
        assert!(Settings::default().home.trim().is_empty());
    }
}
