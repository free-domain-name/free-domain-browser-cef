//! Small JSON settings store shared by the chrome bridge and the settings page.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub home: String,
    pub cookies: bool,
    pub dnt: bool,
    /// True when the tab strip is compact. It follows the layout density, so
    /// there is only one knob for it in the settings.
    pub compact_tabs: bool,
    pub density: String,
    pub theme: String,
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
            custom_css: String::new(),
        }
    }
}

impl Settings {
    /// Clamp the values the interface defines: an unknown theme or density
    /// falls back to the default, the compact tab strip follows the density
    /// and the custom CSS is cut at a character boundary. The same check
    /// runs for a file the user edited by hand.
    fn sanitize(&mut self) {
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            self.theme = Settings::default().theme;
        }
        if !matches!(self.density.as_str(), "comfortable" | "compact") {
            self.density = Settings::default().density;
        }
        // The compact tab strip is the compact layout: one knob, one meaning.
        self.compact_tabs = self.density == "compact";
        if self.custom_css.len() > 24_000 {
            let mut end = 24_000;
            while !self.custom_css.is_char_boundary(end) {
                end -= 1;
            }
            self.custom_css.truncate(end);
        }
    }

    /// Accept the object sent by the settings page. Unknown keys are ignored.
    /// An empty home page is kept: it means the start page.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let mut settings: Settings = serde_json::from_value(value.clone()).ok()?;
        settings.sanitize();
        Some(settings)
    }
}

/// The folder the settings live in, per platform:
/// - macOS: `~/Library/Application Support/Free Domain Browser CEF`
/// - Windows: `%APPDATA%\Free Domain Browser CEF`
/// - Linux: `$XDG_CONFIG_HOME/free-domain-browser-cef`
pub fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("FDB_SETTINGS_PATH") {
        // An explicit settings path is a profile: everything that belongs to
        // the profile lives beside it, so a test run and a real run never
        // share a file.
        let path = PathBuf::from(path);
        return path.parent().map(Path::to_path_buf).unwrap_or(path);
    }
    if let Some(dir) = std::env::var_os("FDB_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    platform_config_dir()
}

/// The folder of the platform, without the overrides. The overrides belong
/// to a run (a test profile), the platform folder belongs to the machine.
fn platform_config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/Free Domain Browser CEF");
    }
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("Free Domain Browser CEF");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(config).join("free-domain-browser-cef");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config/free-domain-browser-cef");
        }
    }
    std::env::temp_dir().join("free-domain-browser-cef")
}

pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("FDB_SETTINGS_PATH") {
        return PathBuf::from(path);
    }
    config_dir().join("settings.json")
}

pub fn load_from(path: &Path) -> Settings {
    let mut settings: Settings = std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    settings.sanitize();
    settings
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

/// The settings are asked for on every request, so the file is parsed once
/// and kept in memory. The stamp of the file says when it changed: a file
/// the user edited by hand is picked up, a file that did not change is not
/// read again.
#[derive(Clone)]
struct Cached {
    path: PathBuf,
    stamp: Option<(std::time::SystemTime, u64)>,
    settings: Settings,
}

fn cache() -> &'static std::sync::Mutex<Option<Cached>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<Cached>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

fn stamp_of(path: &Path) -> Option<(std::time::SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn source_path() -> Option<PathBuf> {
    let primary = default_path();
    if primary.exists() {
        return Some(primary);
    }
    // An explicit settings path is an isolation boundary (tests and parallel
    // profiles). Do not leak the shared fallback file into that profile.
    if std::env::var_os("FDB_SETTINGS_PATH").is_some() {
        return None;
    }
    Some(fallback_path())
}

pub fn load() -> Settings {
    let Some(source) = source_path() else {
        return Settings::default();
    };
    let stamp = stamp_of(&source);
    let mut cache = cache().lock().unwrap_or_else(|error| error.into_inner());
    if let Some(entry) = cache.as_ref()
        && entry.path == source
        && entry.stamp == stamp
    {
        return entry.settings.clone();
    }
    let settings = load_from(&source);
    *cache = Some(Cached {
        path: source,
        stamp,
        settings: settings.clone(),
    });
    settings
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    // The app may be sandboxed without Application Support access (for example
    // during `cargo run`); keep a temp fallback so settings still persist.
    let primary = default_path();
    let (path, result) = match save_to(&primary, settings) {
        Ok(()) => (primary, Ok(())),
        Err(error) => {
            let fallback = fallback_path();
            match save_to(&fallback, settings) {
                Ok(()) => (fallback, Ok(())),
                Err(_) => (primary, Err(error)),
            }
        }
    };
    if result.is_ok() {
        // The file on disk is the truth now: refresh the memory copy with it,
        // so the next read does not wait for the stamp to change.
        let stamp = stamp_of(&path);
        let mut cache = cache().lock().unwrap_or_else(|error| error.into_inner());
        *cache = Some(Cached {
            path,
            stamp,
            settings: settings.clone(),
        });
    }
    result
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

    #[test]
    fn the_compact_tab_strip_follows_the_layout_density() {
        // One knob for the compact layout: the density decides it.
        let compact = Settings::from_value(&serde_json::json!({"density":"compact"})).unwrap();
        assert!(compact.compact_tabs);
        let comfortable =
            Settings::from_value(&serde_json::json!({"density":"comfortable"})).unwrap();
        assert!(!comfortable.compact_tabs);
    }

    #[test]
    fn a_hand_edited_file_is_held_to_the_same_rules() {
        // A file the user edited by hand goes through the same check as the
        // one the settings page sends.
        let dir = std::env::temp_dir().join(format!("fdb-settings-edited-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"theme":"neon","density":"huge","compact_tabs":true,"custom_css":""}"#,
        )
        .unwrap();
        let settings = load_from(&path);
        assert_eq!(settings.theme, Settings::default().theme);
        assert_eq!(settings.density, Settings::default().density);
        assert!(!settings.compact_tabs);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_settings_live_in_the_folder_of_the_platform() {
        // The settings do not belong in a macOS folder on Linux or Windows.
        let dir = platform_config_dir();
        let text = dir.to_string_lossy().to_string();
        #[cfg(target_os = "macos")]
        assert!(text.contains("Library/Application Support"));
        #[cfg(target_os = "windows")]
        assert!(!text.contains("Library"));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(!text.contains("Library/Application Support"), "{text}");
    }

    #[test]
    fn a_changed_file_is_picked_up() {
        // The settings are kept in memory, but a file that changed on disk
        // must still be read.
        let dir = std::env::temp_dir().join(format!("fdb-settings-cache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        // SAFETY: environment mutation is process-global. This test owns the
        // variable for its duration and restores it before returning.
        let previous = std::env::var_os("FDB_SETTINGS_PATH");
        unsafe {
            std::env::set_var("FDB_SETTINGS_PATH", &path);
        }
        save_to(&path, &Settings::default()).unwrap();
        assert_eq!(load().theme, Settings::default().theme);
        let changed = Settings {
            theme: "dark".to_string(),
            ..Settings::default()
        };
        std::fs::write(&path, serde_json::to_string(&changed).unwrap()).unwrap();
        assert_eq!(load().theme, "dark");
        unsafe {
            match previous {
                Some(value) => std::env::set_var("FDB_SETTINGS_PATH", value),
                None => std::env::remove_var("FDB_SETTINGS_PATH"),
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
