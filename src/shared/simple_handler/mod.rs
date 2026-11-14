use cef::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::shared::simple_app::{create_content_view, open_review_window};

pub fn get_data_uri(data: &[u8], mime_type: &str) -> String {
    let data = CefString::from(&base64_encode(Some(data)));
    let uri = CefString::from(&uriencode(Some(&data), 0)).to_string();
    format!("data:{mime_type};base64,{uri}")
}

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac::*;

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;

static SIMPLE_HANDLER_INSTANCE: OnceLock<Weak<Mutex<SimpleHandler>>> = OnceLock::new();

enum BridgeAction {
    Navigate(i32, String),
    Page(i32, String),
    SaveSettings(crate::settings::Settings),
    BrowserCommand(i32, BrowserCommandKind),
    Home(i32),
    NewTab(u64, String),
    CloseTab(u64),
    SelectTab(u64),
    External(String),
    ClearCache,
    Recheck(i32, String),
    OpenSettings,
    CloseSettings,
    ToggleChatPanel,
    CloseChatPanel,
    Review(String),
    About,
    FocusChrome,
    Quit,
}

#[derive(Clone, Copy)]
enum BrowserCommandKind {
    Back,
    Forward,
    Reload,
}

struct Tab {
    uid: u64,
    browser_id: i32,
    view: BrowserView,
}

const CHROME_HEIGHT: i32 = 116;
const CHAT_PANEL_WIDTH: i32 = 420;

pub struct SimpleHandler {
    is_alloy_style: bool,
    browser_list: Vec<Browser>,
    chrome_browser_id: Option<i32>,
    chrome_view: Option<View>,
    settings_view: Option<BrowserView>,
    chat_panel: Option<BrowserView>,
    settings_open: bool,
    chat_open: bool,
    tabs: Vec<Tab>,
    pending_uids: VecDeque<u64>,
    pending_views: Vec<(u64, BrowserView)>,
    active_uid: Option<u64>,
    window: Option<Window>,
    is_closing: bool,
    weak_self: Weak<Mutex<Self>>,
}

const SETTINGS_HTML: &str = include_str!("../../../assets/settings.html");
const CHAT_HTML: &str = include_str!("../../../assets/chat.html");
const CONFLICT_HTML: &str = include_str!("../../../assets/conflict.html");
const CLAIM_HTML: &str = include_str!("../../../assets/claim.html");
const UNAVAILABLE_HTML: &str = include_str!("../../../assets/unavailable.html");
const NEWTAB_HTML: &str = include_str!("../../../assets/newtab.html");
const ERROR_HTML: &str = include_str!("../../../assets/error.html");

const APPEARANCE_JS: &str = r#"<script>(function(){try{var q=new URLSearchParams(location.search);var theme=q.get('theme')||'system';var skin=q.get('skin')||'default';var density=q.get('density')||'comfortable';var dark=theme==='dark'||(theme!=='light'&&window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches);var root=document.documentElement;root.dataset.theme=dark?'dark':'light';root.dataset.skin=skin;root.dataset.density=density;var style=document.createElement('style');style.textContent='html[data-theme=light] body{background:#eef2f8!important;color:#182131!important}html[data-theme=light] aside,html[data-theme=light] header,html[data-theme=light] .card,html[data-theme=light] .box,html[data-theme=light] .search{background:#fff!important;border-color:#d6deea!important;color:#182131!important}html[data-theme=light] .sub,html[data-theme=light] .note,html[data-theme=light] .fineprint,html[data-theme=light] .meta,html[data-theme=light] .empty,html[data-theme=light] .kind{color:#5d687c!important}html[data-theme=light] input,html[data-theme=light] textarea,html[data-theme=light] select{background:#fff!important;color:#111!important;border-color:#c8d1df!important}html[data-theme=light] button{background:#e4e9f1!important;color:#182131!important;border-color:#c8d1df!important}html[data-theme=light] #send,html[data-theme=light] .primary,html[data-theme=light] #ageContinue,html[data-theme=light] .go{background:#2563eb!important;color:#fff!important}html[data-theme=light] .messages,html[data-theme=light] #connection{background:#f7f9fc!important;border-color:#d6deea!important}html[data-theme=light] .msg{background:#fff!important;color:#182131!important;border:1px solid #d6deea}html[data-theme=light] .msg.me{background:#dbe7ff!important}html[data-theme=light] .sample{background:#fff!important;border-color:#d6deea!important;color:#182131!important}html[data-theme=light] pre,html[data-theme=light] code{background:#f3f5f9!important;color:#182131!important}html[data-theme=light] #age,html[data-theme=light] #profileView{background:rgba(238,242,248,.85)!important}html[data-skin=high-tech]{--skin-bg:#0c1114;--skin-panel:#11181d;--skin-line:#1d3a42;--skin-ink:#d8f7ff;--skin-muted:#78a6b2;--skin-accent:#22d3ee;--skin-input:#071014}html[data-skin=futuristic]{--skin-bg:#120b24;--skin-panel:#1b1033;--skin-line:#4c2a7a;--skin-ink:#f3e8ff;--skin-muted:#b39ddb;--skin-accent:#a855f7;--skin-input:#0c0618}html[data-skin=historic]{--skin-bg:#e8dcc3;--skin-panel:#f3ead6;--skin-line:#c4ae8b;--skin-ink:#3b2a18;--skin-muted:#7a644c;--skin-accent:#8b5e34;--skin-input:#fffaf0}html[data-skin=victorian]{--skin-bg:#2a1820;--skin-panel:#3a2029;--skin-line:#6e4350;--skin-ink:#f4e6d0;--skin-muted:#c9a98d;--skin-accent:#b8860b;--skin-input:#1d1016}html[data-skin=amazon]{--skin-bg:#10251b;--skin-panel:#173326;--skin-line:#2f5e45;--skin-ink:#e8f5e9;--skin-muted:#9cc3a8;--skin-accent:#e07a3f;--skin-input:#0a1a12}html[data-skin=terminal]{--skin-bg:#050805;--skin-panel:#071007;--skin-line:#1f4a1f;--skin-ink:#b8ffb8;--skin-muted:#5f9f5f;--skin-accent:#22c55e;--skin-input:#020402}html[data-skin]:not([data-skin=default]) body{background:var(--skin-bg)!important;color:var(--skin-ink)!important}html[data-skin]:not([data-skin=default]) aside,html[data-skin]:not([data-skin=default]) header,html[data-skin]:not([data-skin=default]) .card,html[data-skin]:not([data-skin=default]) .box,html[data-skin]:not([data-skin=default]) .search{background:var(--skin-panel)!important;border-color:var(--skin-line)!important;color:var(--skin-ink)!important}html[data-skin]:not([data-skin=default]) .sub,html[data-skin]:not([data-skin=default]) .note,html[data-skin]:not([data-skin=default]) .fineprint,html[data-skin]:not([data-skin=default]) .meta,html[data-skin]:not([data-skin=default]) .empty{color:var(--skin-muted)!important}html[data-skin]:not([data-skin=default]) input,html[data-skin]:not([data-skin=default]) textarea,html[data-skin]:not([data-skin=default]) select{background:var(--skin-input)!important;color:var(--skin-ink)!important;border-color:var(--skin-line)!important}html[data-skin]:not([data-skin=default]) button{background:var(--skin-panel)!important;color:var(--skin-ink)!important;border-color:var(--skin-line)!important}html[data-skin]:not([data-skin=default]) #send,html[data-skin]:not([data-skin=default]) .primary,html[data-skin]:not([data-skin=default]) #ageContinue,html[data-skin]:not([data-skin=default]) .go{background:var(--skin-accent)!important;color:#fff!important}';document.head.appendChild(style)}catch(e){}})();</script>"#;

/// Materialize an internal page on disk so it gets a real file:// origin and
/// can use localStorage for settings.
fn internal_page_url(html: &str) -> String {
    let dir = std::env::temp_dir().join("free-domain-browser-cef-ui");
    let _ = std::fs::create_dir_all(&dir);
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut hasher);
    let path = dir.join(format!("{:x}.html", hasher.finish()));
    // Every internal page gets the same appearance bootstrap. The query string
    // carries theme/skin/density from the Rust settings store.
    let themed = match html.find("</head>") {
        Some(index) => {
            let mut value = String::with_capacity(html.len() + APPEARANCE_JS.len());
            value.push_str(&html[..index]);
            value.push_str(APPEARANCE_JS);
            value.push_str(&html[index..]);
            value
        }
        None => html.to_string(),
    };
    let _ = std::fs::write(&path, &themed);
    let settings = crate::settings::load();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("theme", &settings.theme)
        .append_pair("skin", &settings.skin)
        .append_pair("density", &settings.density)
        .finish();
    url::Url::from_file_path(&path)
        .map(|url| format!("{url}?{query}"))
        .unwrap_or_else(|_| get_data_uri(themed.as_bytes(), "text/html"))
}

fn settings_page_url(settings: &crate::settings::Settings) -> String {
    let base = internal_page_url(SETTINGS_HTML);
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("home", &settings.home)
        .append_pair("cookies", if settings.cookies { "1" } else { "0" })
        .append_pair("dnt", if settings.dnt { "1" } else { "0" })
        .append_pair(
            "compact_tabs",
            if settings.compact_tabs { "1" } else { "0" },
        )
        .append_pair("density", &settings.density)
        .append_pair("show_status", if settings.show_status { "1" } else { "0" })
        .append_pair("theme", &settings.theme)
        .append_pair("skin", &settings.skin)
        .append_pair("custom_css", &settings.custom_css)
        .finish();
    format!("{base}&{query}")
}

fn choice_url(domain: &str, target: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("domain", domain)
        .append_pair("target", target)
        .finish();
    format!("https://fdb.invalid/choose?{query}")
}

fn navigation_key(input: &str) -> String {
    let mut value = input.trim().to_ascii_lowercase();
    if let Some(rest) = value.strip_prefix("fdn://") {
        value = rest.to_string();
    } else if let Ok(parsed) = url::Url::parse(&value) {
        if matches!(parsed.scheme(), "http" | "https" | "fdn") {
            if let Some(host) = parsed.host_str() {
                value = host.to_string();
            }
        }
    }
    value
        .trim_end_matches('/')
        .trim_end_matches(".fdn")
        .to_string()
}

fn remembered_choice(input: &str) -> Option<String> {
    static CHOICES: std::sync::OnceLock<std::sync::Mutex<HashMap<String, String>>> =
        std::sync::OnceLock::new();
    let choices = CHOICES.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let choices = choices.lock().ok()?;
    choices.get(&navigation_key(input)).cloned()
}

fn remember_choice(domain: &str, target: &str) {
    static CHOICES: std::sync::OnceLock<std::sync::Mutex<HashMap<String, String>>> =
        std::sync::OnceLock::new();
    let choices = CHOICES.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(mut choices) = choices.lock() {
        choices.insert(navigation_key(domain), target.to_string());
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn error_page(failed_url: &str, error_text: &str, error_code: i32) -> String {
    ERROR_HTML
        .replace("{{url}}", &html_escape(failed_url))
        .replace("{{error}}", &html_escape(error_text))
        .replace("{{code}}", &error_code.to_string())
}

fn conflict_page(outcome: &fdn_core::NavigationOutcome) -> String {
    let target = crate::proxy::proxy_url(&outcome.domain, &outcome.path)
        .or_else(|| outcome.registry_target.clone())
        .unwrap_or_default();
    let public = outcome.public_url.clone().unwrap_or_default();
    CONFLICT_HTML
        .replace("{{domain}}", &outcome.domain)
        .replace("{{registry_choice}}", &choice_url(&outcome.domain, &target))
        .replace("{{public_choice}}", &choice_url(&outcome.domain, &public))
        .replace("{{target}}", &target)
}

fn claim_page(outcome: &fdn_core::NavigationOutcome) -> String {
    let path = fdn_core::RegistryClient::shard_path(&outcome.domain);
    let record = serde_json::json!({
        "domain": outcome.domain,
        "target_url": "http://203.0.113.10:8080",
        "owner_github": "your-github-handle",
        "ttl": 3600,
    });
    let json = serde_json::to_string_pretty(&record).unwrap_or_default() + "\n";
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("filename", &path)
        .append_pair("value", &json)
        .finish();
    let register_url = format!(
        "https://github.com/free-domain-name/registry/new/main?{query}"
    );
    let free_namespace = outcome.domain == "fdn" || outcome.domain.ends_with(".fdn");
    let badge = if free_namespace { "Available" } else { "Unregistered" };
    let public_row = if free_namespace {
        String::new()
    } else {
        format!(
            "<p style=\"margin-top:22px\"><a class=\"button secondary\" href=\"{}\">Try public website</a></p>",
            html_escape(
                outcome
                    .public_url
                    .as_deref()
                    .unwrap_or("https://github.com/free-domain-name/registry"),
            )
        )
    };
    let domain_js = serde_json::to_string(&outcome.domain).unwrap_or_else(|_| "\"\"".into());
    CLAIM_HTML
        .replace("{{domain}}", &html_escape(&outcome.domain))
        .replace("{{domain_js}}", &domain_js)
        .replace("{{badge}}", badge)
        .replace(
            "{{message}}",
            &html_escape(
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("This name is not registered yet."),
            ),
        )
        .replace("{{path}}", &html_escape(&path))
        .replace("{{json}}", &html_escape(&json))
        .replace("{{register_url}}", &html_escape(&register_url))
        .replace(
            "{{repo_url}}",
            "https://github.com/free-domain-name/registry",
        )
        .replace("{{public_row}}", &public_row)
}

fn unavailable_page(outcome: &fdn_core::NavigationOutcome) -> String {
    let free_namespace = outcome.domain == "fdn" || outcome.domain.ends_with(".fdn");
    let public_url = outcome.public_url.clone().unwrap_or_default();
    let public_button = if free_namespace || public_url.is_empty() {
        String::new()
    } else {
        format!(
            r#"<button id="public" class="secondary" type="button">Continue to public web</button>"#
        )
    };
    let domain_js = serde_json::to_string(&outcome.domain).unwrap_or_else(|_| "\"\"".into());
    let public_js = serde_json::to_string(&public_url).unwrap_or_else(|_| "\"\"".into());
    UNAVAILABLE_HTML
        .replace("{{domain}}", &html_escape(&outcome.domain))
        .replace("{{domain_js}}", &domain_js)
        .replace(
            "{{message}}",
            &html_escape(
                outcome
                    .message
                    .as_deref()
                    .unwrap_or("The registry request failed."),
            ),
        )
        .replace("{{public_button}}", &public_button)
        .replace(
            "{{free_note}}",
            if free_namespace {
                "Public DNS cannot answer .fdn names, so retry when the registry is reachable."
            } else {
                "Retry, or continue explicitly to the public web."
            },
        )
        .replace("{{public_js}}", &public_js)
}

fn choice_target(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.host_str() != Some("fdb.invalid") || parsed.path() != "/choose" {
        return None;
    }
    let mut domain = None;
    let mut target = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "domain" => domain = Some(value.into_owned()),
            "target" => target = Some(value.into_owned()),
            _ => {}
        }
    }
    let target = target?;
    if !matches!(url::Url::parse(&target).ok()?.scheme(), "http" | "https") {
        return None;
    }
    Some((domain.unwrap_or_default(), target))
}

/// Parse the synthetic URL used by the chrome bar to request page navigation.
fn navigation_target(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.host_str() != Some("fdb.invalid") || parsed.path() != "/navigate" {
        return None;
    }
    parsed
        .query_pairs()
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.into_owned())
}

impl SimpleHandler {
    pub fn instance() -> Option<Arc<Mutex<Self>>> {
        SIMPLE_HANDLER_INSTANCE
            .get()
            .and_then(|weak| weak.upgrade())
    }

    pub fn mark_chrome_browser(&mut self, browser_id: i32) {
        self.chrome_browser_id = Some(browser_id);
    }

    pub fn new(is_alloy_style: bool) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|weak| {
            if let Err(instance) = SIMPLE_HANDLER_INSTANCE.set(weak.clone()) {
                assert_eq!(instance.strong_count(), 0, "Replacing a viable instance");
            }

            Mutex::new(Self {
                is_alloy_style,
                browser_list: Vec::new(),
                chrome_browser_id: None,
                chrome_view: None,
                settings_view: None,
                chat_panel: None,
                settings_open: false,
                chat_open: false,
                tabs: Vec::new(),
                pending_uids: VecDeque::new(),
                pending_views: Vec::new(),
                active_uid: None,
                window: None,
                is_closing: false,
                weak_self: weak.clone(),
            })
        })
    }

    fn on_title_change(&mut self, browser: Option<&mut Browser>, title: Option<&CefString>) {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        let mut browser = browser.cloned();
        // Only real tabs (and the chrome) own the main window title. Overlay
        // settings/chat views and review windows must not rename the browser.
        let owns_main_title = browser
            .as_ref()
            .is_some_and(|browser| self.tab_uid(browser.identifier()).is_some());
        if owns_main_title {
            if let Some(browser_view) = browser_view_get_for_browser(browser.as_mut()) {
                if let Some(window) = browser_view.window() {
                    window.set_title(title);
                }
            } else if self.is_alloy_style {
                platform_title_change(browser.as_mut(), title);
            }
        }
        if let Some(browser) = browser.as_ref() {
            if self.chrome_browser_id != Some(browser.identifier()) {
                let title = title.map(CefString::to_string).unwrap_or_default();
                self.update_chrome(browser.identifier(), None, Some(&title));
            }
        }
    }

    fn update_chrome(&mut self, source_id: i32, url: Option<&str>, title: Option<&str>) {
        let Some(chrome_id) = self.chrome_browser_id else { return; };
        if source_id == chrome_id {
            return;
        }
        if url.is_some_and(|value| value.starts_with("file://") || value.starts_with("data:")) {
            return;
        }
        let Some(uid) = self.tab_uid(source_id) else { return; };
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else { return; };
        let Some(frame) = chrome.main_frame() else { return; };
        let url = url.map(|value| serde_json::to_string(value).unwrap_or_default());
        let title = title.map(|value| serde_json::to_string(value).unwrap_or_default());
        let script = match (url, title) {
            (Some(url), Some(title)) => format!("window.fdbUpdate&&fdbUpdate({url},{title},{uid})"),
            (Some(url), None) => format!("window.fdbUpdate&&fdbUpdate({url},null,{uid})"),
            (None, Some(title)) => format!("window.fdbUpdate&&fdbUpdate(null,{title},{uid})"),
            (None, None) => return,
        };
        let mut frame = frame;
        frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
    }

    fn chrome_toast(&self, text: &str) {
        let message = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
        let script = format!("window.fdbStatus&&fdbStatus({message})");
        self.chrome_execute(0, &script);
    }

    fn apply_settings(&self) {
        let settings = crate::settings::load();
        let payload = serde_json::json!({
            "compact_tabs": settings.compact_tabs,
            "density": settings.density,
            "show_status": settings.show_status,
            "theme": settings.theme,
            "skin": settings.skin,
            "custom_css": settings.custom_css,
        });
        let script = format!("window.fdbSettings&&fdbSettings({payload})");
        self.chrome_execute(0, &script);
    }

    fn chrome_execute(&self, source_id: i32, script: &str) {
        let Some(chrome_id) = self.chrome_browser_id else { return; };
        if source_id == chrome_id { return; }
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else { return; };
        let Some(frame) = chrome.main_frame() else { return; };
        let mut frame = frame;
        frame.execute_java_script(Some(&CefString::from(script)), None, 0);
    }

    fn chrome_focus(&self, script: &str) {
        let Some(chrome_id) = self.chrome_browser_id else { return; };
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else { return; };
        if let Some(host) = chrome.host() {
            host.set_focus(1);
        }
        if let Some(frame) = chrome.main_frame() {
            let mut frame = frame;
            frame.execute_java_script(Some(&CefString::from(script)), None, 0);
        }
    }

    fn tab(&self, uid: u64) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.uid == uid)
    }

    fn tab_uid(&self, browser_id: i32) -> Option<u64> {
        self.tabs
            .iter()
            .find(|tab| tab.browser_id == browser_id)
            .map(|tab| tab.uid)
    }

    pub fn register_initial_tabs(&mut self, views: &[BrowserView]) {
        for (uid, view) in views.iter().enumerate() {
            self.register_tab(uid as u64, view.clone());
        }
        if self.active_uid.is_none() {
            self.active_uid = self.tabs.first().map(|tab| tab.uid);
        }
    }

    pub fn register_tab(&mut self, uid: u64, view: BrowserView) {
        if self.tabs.iter().any(|tab| tab.uid == uid) {
            return;
        }
        let Some(browser) = view.browser() else { return; };
        self.tabs.push(Tab {
            uid,
            browser_id: browser.identifier(),
            view,
        });
    }

    pub fn set_window(&mut self, window: Window) {
        self.window = Some(window);
    }

    pub fn set_chrome_view(&mut self, view: BrowserView) {
        self.chrome_view = Some(View::from(&view));
    }

    fn tab_bounds(window: &Window) -> Rect {
        let bounds = window.bounds();
        Rect {
            x: 0,
            y: CHROME_HEIGHT.min(bounds.height.max(1)),
            width: bounds.width.max(1),
            height: (bounds.height - CHROME_HEIGHT).max(1),
        }
    }

    pub fn resize_tabs(&mut self, _bounds: &Rect) {
        let Some(window) = self.window.clone() else { return; };
        let bounds = Self::tab_bounds(&window);
        for tab in &self.tabs {
            let mut view = View::from(&tab.view);
            view.set_bounds(Some(&bounds));
        }
        self.layout_overlays(&window);
    }

    fn content_bounds(window: &Window) -> Rect {
        let bounds = window.bounds();
        Rect {
            x: 0,
            y: CHROME_HEIGHT.min(bounds.height.max(1)),
            width: bounds.width.max(1),
            height: (bounds.height - CHROME_HEIGHT).max(1),
        }
    }

    fn chat_bounds(window: &Window) -> Rect {
        let bounds = Self::content_bounds(window);
        let width = CHAT_PANEL_WIDTH.min(bounds.width.max(1));
        Rect {
            x: (bounds.width - width).max(0),
            y: bounds.y,
            width,
            height: bounds.height.max(1),
        }
    }

    fn layout_overlays(&self, window: &Window) {
        if let Some(view) = self.settings_view.as_ref() {
            let mut view = View::from(view);
            view.set_bounds(Some(&Self::content_bounds(window)));
        }
        if let Some(view) = self.chat_panel.as_ref() {
            let mut view = View::from(view);
            view.set_bounds(Some(&Self::chat_bounds(window)));
        }
    }

    pub fn attach_settings_view(&mut self, view: BrowserView) {
        let Some(window) = self.window.clone() else { return; };
        if self.chat_open {
            self.close_chat_panel();
        }
        self.settings_view = Some(view);
        self.settings_open = true;
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbSettingsOpen&&fdbSettingsOpen(true)");
        if let Some(browser) = self.settings_view.as_ref().and_then(|view| view.browser()) {
            if let Some(host) = browser.host() {
                host.set_focus(1);
            }
        }
    }

    pub fn show_settings_view(&mut self) {
        let Some(window) = self.window.clone() else { return; };
        if self.chat_open {
            self.close_chat_panel();
        }
        if let Some(view) = self.settings_view.as_ref() {
            let mut view = View::from(view);
            view.set_visible(1);
        }
        self.settings_open = true;
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbSettingsOpen&&fdbSettingsOpen(true)");
    }

    pub fn close_settings_view(&mut self) {
        if let Some(view) = self.settings_view.as_ref() {
            let mut view = View::from(view);
            view.set_visible(0);
        }
        self.settings_open = false;
        self.chrome_execute(0, "window.fdbSettingsOpen&&fdbSettingsOpen(false)");
    }

    pub fn attach_chat_panel(&mut self, view: BrowserView) {
        let Some(window) = self.window.clone() else { return; };
        if self.settings_open {
            self.close_settings_view();
        }
        self.chat_panel = Some(view);
        self.chat_open = true;
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(true)");
        if let Some(browser) = self.chat_panel.as_ref().and_then(|view| view.browser()) {
            if let Some(host) = browser.host() {
                host.set_focus(1);
            }
            if let Some(frame) = browser.main_frame() {
                let mut frame = frame;
                frame.execute_java_script(Some(&CefString::from("window.fdbChatPanelOpen&&fdbChatPanelOpen()")), None, 0);
            }
        }
    }

    pub fn show_chat_panel(&mut self) {
        let Some(window) = self.window.clone() else { return; };
        if self.settings_open {
            self.close_settings_view();
        }
        if let Some(view) = self.chat_panel.as_ref() {
            let mut view = View::from(view);
            view.set_visible(1);
        }
        self.chat_open = true;
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(true)");
        if let Some(browser) = self.chat_panel.as_ref().and_then(|view| view.browser()) {
            if let Some(host) = browser.host() {
                host.set_focus(1);
            }
        }
    }

    pub fn close_chat_panel(&mut self) {
        if let Some(view) = self.chat_panel.as_ref() {
            if let Some(browser) = view.browser() {
                if let Some(frame) = browser.main_frame() {
                    let mut frame = frame;
                    frame.execute_java_script(Some(&CefString::from("window.fdbPanelClose&&fdbPanelClose()")), None, 0);
                }
            }
            let mut view = View::from(view);
            view.set_visible(0);
        }
        self.chat_open = false;
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(false)");
    }

    fn chrome_state(&self) {
        self.chrome_execute(0, &format!("window.fdbSettingsOpen&&fdbSettingsOpen({})", self.settings_open));
        self.chrome_execute(0, &format!("window.fdbChatPanel&&fdbChatPanel({})", self.chat_open));
    }

    fn switch_uid(&mut self, uid: u64) {
        if self.tab(uid).is_none() {
            return;
        }
        self.active_uid = Some(uid);
        for tab in &self.tabs {
            let mut view = View::from(&tab.view);
            view.set_visible(i32::from(tab.uid == uid));
        }
    }

    fn active_browser_id(&self) -> Option<i32> {
        let uid = self.active_uid?;
        self.tab(uid).map(|tab| tab.browser_id)
    }

    fn take_tab(&mut self, uid: u64) -> Option<(BrowserView, Option<Browser>)> {
        let index = self.tabs.iter().position(|tab| tab.uid == uid)?;
        let tab = self.tabs.remove(index);
        if self.active_uid == Some(uid) {
            self.active_uid = self
                .tabs
                .get(index.min(self.tabs.len().saturating_sub(1)))
                .map(|tab| tab.uid);
        }
        let browser = tab.view.browser();
        Some((tab.view, browser))
    }

    fn on_status_message(&mut self, browser: Option<&mut Browser>, value: Option<&CefString>) {
        let Some(browser) = browser else { return; };
        let source_id = browser.identifier();
        if self.tab_uid(source_id).is_none() { return; }
        let text = value.map(CefString::to_string).unwrap_or_default();
        let script = format!(
            "window.fdbStatus&&fdbStatus({})",
            serde_json::to_string(&text).unwrap_or_default()
        );
        self.chrome_execute(source_id, &script);
    }

    fn on_loading_state_change(
        &mut self,
        browser: Option<&mut Browser>,
        is_loading: bool,
        can_go_back: bool,
        can_go_forward: bool,
    ) {
        let Some(browser) = browser else { return; };
        let source_id = browser.identifier();
        let Some(uid) = self.tab_uid(source_id) else { return; };
        let script = format!(
            "window.fdbLoad&&fdbLoad({},{},{},{})",
            if is_loading { 1 } else { 0 },
            if can_go_back { 1 } else { 0 },
            if can_go_forward { 1 } else { 0 },
            uid
        );
        self.chrome_execute(source_id, &script);
    }

    fn on_after_created(&mut self, browser: Option<&mut Browser>) {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        let browser = browser.cloned().expect("Browser is None");

        // Sanity-check the configured runtime style.
        assert_eq!(
            browser.host().expect("BrowserHost is None").runtime_style(),
            if self.is_alloy_style {
                RuntimeStyle::ALLOY
            } else {
                RuntimeStyle::CHROME
            }
        );

        // CEF may create the content browsers before the chrome browser. Use the
        // page URL to identify the chrome view instead of creation order.
        let mut browser = browser;
        if Self::is_chrome_frame(&mut browser) {
            self.chrome_browser_id = Some(browser.identifier());
            self.browser_list.push(browser);
            return;
        }

        self.browser_list.push(browser.clone());

        // Identify the fixed chrome view by identity, not by URL or creation
        // order: the URL is still empty during `on_after_created`.
        if let (Some(chrome), Some(mut view)) = (
            self.chrome_view.as_mut(),
            browser_view_get_for_browser(Some(&mut browser)),
        ) {
            let mut candidate = View::from(&view);
            if chrome.is_same(Some(&mut candidate)) != 0 {
                self.chrome_browser_id = Some(browser.identifier());
                return;
            }
        }

        if let Some(uid) = self.pending_uids.front().copied() {
            if let Some(position) = self.pending_views.iter().position(|(pending, _)| *pending == uid) {
                let (_, view) = self.pending_views.remove(position);
                self.pending_uids.pop_front();
                self.register_tab(uid, view);
                self.switch_uid(uid);
            }
            return;
        }

        // Initial tab. The browser may not exist while `on_window_created` runs,
        // so register the first page browser as soon as CEF reports it.
        if self.tabs.is_empty() {
            if let Some(view) = browser_view_get_for_browser(Some(&mut browser)) {
                self.register_tab(0, view);
                self.active_uid = Some(0);
            }
        }
    }

    fn do_close(&mut self, _browser: Option<&mut Browser>) -> bool {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        // Closing the main window requires special handling. See the DoClose()
        // documentation in the CEF header for a detailed destription of this
        // process.
        if self.browser_list.len() == 1 {
            // Set a flag to indicate that the window close should be allowed.
            self.is_closing = true;
        }

        // Allow the close. For windowed browsers this will result in the OS close
        // event being sent.
        false
    }

    fn on_before_close(&mut self, browser: Option<&mut Browser>) {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        // Remove from the list of existing browsers.
        let mut browser = browser.cloned().expect("Browser is None");
        let browser_id = browser.identifier();
        if let Some(index) = self
            .browser_list
            .iter()
            .position(|elem| elem.is_same(Some(&mut browser)) != 0)
        {
            self.browser_list.remove(index);
        }
        if self.tabs.iter().any(|tab| tab.browser_id == browser_id) {
            self.tabs.retain(|tab| tab.browser_id != browser_id);
            if self
                .active_uid
                .is_some_and(|uid| self.tab(uid).is_none())
            {
                self.active_uid = self.tabs.last().map(|tab| tab.uid);
            }
        }

        if self.browser_list.is_empty() {
            // All browser windows have closed. Quit the application message loop.
            quit_message_loop();
        }
    }

    fn on_load_start(
        &mut self,
        browser: Option<&mut Browser>,
        frame: Option<&mut Frame>,
        _transition_type: TransitionType,
    ) {
        let (Some(browser), Some(frame)) = (browser, frame) else { return; };
        let source_id = browser.identifier();
        let url = CefString::from(&frame.url()).to_string();
        if url.starts_with("data:text/html") {
            self.chrome_browser_id = Some(source_id);
            // The chrome browser may have been registered as an initial tab
            // before its URL was known. Remove it now; it is not a page tab.
            let removed = self.tabs.iter().any(|tab| tab.browser_id == source_id);
            if removed {
                self.tabs.retain(|tab| tab.browser_id != source_id);
                if self.active_uid.is_some_and(|uid| self.tab(uid).is_none()) {
                    self.active_uid = self.tabs.first().map(|tab| tab.uid);
                }
            }
            return;
        }
        if self.tab_uid(source_id).is_some() || !self.tabs.is_empty() {
            return;
        }
        if let Some(view) = browser_view_get_for_browser(Some(browser)) {
            let uid = self.tabs.len() as u64;
            self.register_tab(uid, view);
            if self.active_uid.is_none() {
                self.active_uid = Some(uid);
            }
        }
    }


    fn on_before_popup(
        &mut self,
        browser: Option<&mut Browser>,
        target_url: Option<&CefString>,
    ) -> i32 {
        let Some(browser) = browser else { return 0; };
        if self.chrome_browser_id == Some(browser.identifier()) {
            return 0;
        }
        let Some(target_url) = target_url else { return 0; };
        let url = target_url.to_string();
        if url.is_empty()
            || !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("fdn://"))
        {
            return 0;
        }
        if let Some(instance) = SimpleHandler::instance() {
            let payload = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".to_string());
            let script = format!("window.fdbNewTabUrl&&fdbNewTabUrl({payload})");
            if let Ok(inner) = instance.lock() {
                inner.chrome_execute(0, &script);
            }
        }
        1
    }

    fn on_load_end(
        &mut self,
        browser: Option<&mut Browser>,
        _frame: Option<&mut Frame>,
        _http_status_code: i32,
    ) {
        let Some(browser) = browser else { return; };
        if self.chrome_browser_id != Some(browser.identifier()) {
            return;
        }
        self.apply_settings();
    }

    fn on_load_error(
        &mut self,
        _browser: Option<&mut Browser>,
        frame: Option<&mut Frame>,
        error_code: Errorcode,
        error_text: Option<&CefString>,
        failed_url: Option<&CefString>,
    ) {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        let failed = failed_url.map(CefString::to_string).unwrap_or_default();
        let text = error_text.map(CefString::to_string).unwrap_or_default();
        let code = sys::cef_errorcode_t::from(error_code);
        eprintln!("[cef] load error {}: {text} ({failed})", code as i32);

        // Allow Chrome to show the error page.
        if !self.is_alloy_style {
            return;
        }

        // Don't display an error for downloaded files.
        let error_code = sys::cef_errorcode_t::from(error_code);
        if error_code == sys::cef_errorcode_t::ERR_ABORTED {
            return;
        }
        let error_code = error_code as i32;

        let Some(frame) = frame else { return; };
        let uri = internal_page_url(&error_page(&failed, &text, error_code));
        let uri = CefString::from(uri.as_str());
        frame.load_url(Some(&uri));
    }

    /// Intercept the chrome bar's synthetic navigation and drive the page view.
    /// Kept as a fallback for old links; the console bridge below is the
    /// primary chrome -> page-view command path.
    fn on_before_browse(
        &mut self,
        browser: Option<&mut Browser>,
        _frame: Option<&mut Frame>,
        request: Option<&mut Request>,
        _user_gesture: i32,
        _is_redirect: i32,
    ) -> i32 {
        let Some(request) = request else { return 0; };
        let url = CefString::from(&request.url()).to_string();
        let Some(mut current) = browser else { return 0; };
        let current_id = current.identifier();
        if self.chrome_browser_id != Some(current_id) {
            if let Some((domain, target)) = choice_target(&url) {
                remember_choice(&domain, &target);
                post_page(current_id, target);
                return 1;
            }
            if url.starts_with("fdn://") {
                spawn_navigation(current_id, url);
                return 1;
            }
            return 0;
        }
        let Some(target_id) = self.active_browser_id() else {
            return 0;
        };
        if url == "https://fdb.invalid/settings" {
            post_page(target_id, internal_page_url(SETTINGS_HTML));
            return 1;
        }
        if url == "https://fdb.invalid/chat" {
            post_page(target_id, internal_page_url(CHAT_HTML));
            return 1;
        }
        let Some(target) = navigation_target(&url) else { return 0; };
        spawn_navigation(target_id, target);
        1
    }

    fn is_chrome_frame(browser: &mut Browser) -> bool {
        browser
            .main_frame()
            .map(|frame| {
                CefString::from(&frame.url())
                    .to_string()
                    .starts_with("data:text/html")
            })
            .unwrap_or(false)
    }

    /// Parse a chrome command without executing it. The caller must drop the
    /// handler lock before loading a page, because CEF re-enters navigation
    /// handlers synchronously.
    /// The chrome document and the internal settings/chat pages are trusted UI.
    /// Content pages never get to speak the `fdb:` bridge.
    fn is_ui_frame(browser: &mut Browser) -> bool {
        browser
            .main_frame()
            .map(|frame| {
                let url = CefString::from(&frame.url()).to_string();
                url.starts_with("data:text/html")
                    || (url.starts_with("file://")
                        && url.contains("/free-domain-browser-cef-ui/"))
            })
            .unwrap_or(false)
    }

    fn bridge_action(&mut self, browser: &mut Browser, message: &str) -> Option<BridgeAction> {
        if self.chrome_browser_id != Some(browser.identifier()) && !Self::is_ui_frame(browser) {
            return None;
        }
        let payload = message.strip_prefix("fdb:")?;
        let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
        let target_id = self.active_browser_id()?;
        match value.get("cmd").and_then(|cmd| cmd.as_str())? {
            "navigate" => {
                let url = value.get("url")?.as_str()?.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(BridgeAction::Navigate(target_id, url.to_string()))
                }
            }
            "settings" => Some(BridgeAction::OpenSettings),
            "close_settings" => Some(BridgeAction::CloseSettings),
            "save_settings" => {
                crate::settings::Settings::from_value(&value).map(BridgeAction::SaveSettings)
            }
            "chat" => Some(BridgeAction::ToggleChatPanel),
            "close_chat_panel" => Some(BridgeAction::CloseChatPanel),
            "about" => Some(BridgeAction::About),
            "review" => {
                let domain = value.get("domain")?.as_str()?.trim();
                if domain.is_empty() {
                    None
                } else {
                    Some(BridgeAction::Review(domain.to_string()))
                }
            }
            "new_tab" => {
                let uid = value
                    .get("uid")
                    .and_then(|uid| uid.as_u64())
                    .or_else(|| value.get("view").and_then(|uid| uid.as_u64()))
                    .or_else(|| value.get("index").and_then(|uid| uid.as_u64()))?;
                let url = value
                    .get("url")
                    .and_then(|url| url.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(BridgeAction::NewTab(uid, url))
            }
            "close_tab" => {
                let uid = value
                    .get("uid")
                    .and_then(|uid| uid.as_u64())
                    .or_else(|| value.get("view").and_then(|uid| uid.as_u64()))
                    .or_else(|| value.get("index").and_then(|uid| uid.as_u64()))?;
                Some(BridgeAction::CloseTab(uid))
            }
            "select_tab" => value
                .get("uid")
                .and_then(|uid| uid.as_u64())
                .or_else(|| value.get("view").and_then(|uid| uid.as_u64()))
                .or_else(|| value.get("index").and_then(|uid| uid.as_u64()))
                .map(BridgeAction::SelectTab),
            "back" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Back)),
            "forward" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Forward)),
            "reload" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Reload)),
            "home" => Some(BridgeAction::Home(target_id)),
            "external" => {
                let url = value.get("url")?.as_str()?.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(BridgeAction::External(url.to_string()))
                }
            }
            "clear_cache" => Some(BridgeAction::ClearCache),
            "recheck" => {
                let domain = value.get("domain")?.as_str()?.trim();
                if domain.is_empty() {
                    None
                } else {
                    Some(BridgeAction::Recheck(target_id, domain.to_string()))
                }
            }
            "focus_chrome" => Some(BridgeAction::FocusChrome),
            "quit" => Some(BridgeAction::Quit),
            _ => None,
        }
    }

    pub fn show_main_window(&mut self) {
        let thread_id = ThreadId::UI;
        if currently_on(thread_id) == 0 {
            // Execute on the UI thread.
            let this = self
                .weak_self
                .upgrade()
                .expect("Weak reference to SimpleHandler is None");
            let mut task = ShowMainWindow::new(this);
            post_task(thread_id, Some(&mut task));
            return;
        }

        let Some(mut main_browser) = self.browser_list.first().cloned() else {
            return;
        };

        if let Some(browser_view) = browser_view_get_for_browser(Some(&mut main_browser)) {
            // Show the window using the Views framework.
            if let Some(window) = browser_view.window() {
                window.show();
            }
        } else if self.is_alloy_style {
            platform_show_window(Some(&mut main_browser));
        }
    }

    pub fn close_all_browsers(&mut self, force_close: bool) {
        let thread_id = ThreadId::UI;
        let this = self
            .weak_self
            .upgrade()
            .expect("Weak reference to SimpleHandler is None");
        let mut task = CloseAllBrowsers::new(this, force_close);
        post_task(thread_id, Some(&mut task));
    }

    pub fn is_closing(&self) -> bool {
        self.is_closing
    }
}

wrap_client! {
    pub struct SimpleHandlerClient {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(SimpleHandlerDisplayHandler::new(self.inner.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(SimpleHandlerLifeSpanHandler::new(self.inner.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(SimpleHandlerLoadHandler::new(self.inner.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(SimpleHandlerRequestHandler::new(self.inner.clone()))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(SimpleDownloadHandler::new())
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(SimpleHandlerKeyboardHandler::new(self.inner.clone()))
        }
    }
}


wrap_download_handler! {
    struct SimpleDownloadHandler;

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> i32 {
            1
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            _download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> i32 {
            let Some(callback) = callback else { return 0; };
            let suggested = suggested_name
                .map(CefString::to_string)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "download".to_string());
            let path = unique_download_path(&suggested);
            let cef_path = CefString::from(path.to_string_lossy().as_ref());
            callback.cont(Some(&cef_path), 0);
            if let Some(instance) = SimpleHandler::instance() {
                if let Ok(mut inner) = instance.lock() {
                    inner.chrome_toast(&format!("Downloading to {}", path.display()));
                }
            }
            1
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            _callback: Option<&mut DownloadItemCallback>,
        ) {
            let Some(item) = download_item else { return; };
            let percent = item.percent_complete();
            let done = item.is_complete() != 0;
            if let Some(instance) = SimpleHandler::instance() {
                if let Ok(mut inner) = instance.lock() {
                    if done {
                        inner.chrome_toast("Download complete");
                    } else {
                        inner.chrome_toast(&format!("Downloading… {percent}%"));
                    }
                }
            }
        }
    }
}

fn unique_download_path(suggested: &str) -> std::path::PathBuf {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let directory = home
        .as_ref()
        .map(|home| home.join("Downloads"))
        .filter(|directory| directory.is_dir())
        .or_else(|| home.clone())
        .unwrap_or_else(std::env::temp_dir);
    let base = std::path::Path::new(suggested)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "download".to_string());
    let candidate = directory.join(&base);
    if !candidate.exists() {
        return candidate;
    }
    let path = std::path::Path::new(&base);
    let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("download");
    let extension = path.extension().and_then(|extension| extension.to_str());
    for index in 1..1000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(base)
}

wrap_display_handler! {
    struct SimpleHandlerDisplayHandler {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_title_change(browser, title);
        }

        fn on_status_message(&self, browser: Option<&mut Browser>, value: Option<&CefString>) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_status_message(browser, value);
        }

        fn on_address_change(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            let (Some(browser), Some(url)) = (browser, url) else { return; };
            let id = browser.identifier();
            let url = url.to_string();
            let url = crate::proxy::display_url(&url).unwrap_or(url);
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.update_chrome(id, Some(&url), None);
        }

        fn on_console_message(
            &self,
            browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: i32,
        ) -> i32 {
            let (Some(mut browser), Some(message)) = (browser.cloned(), message.map(CefString::to_string)) else {
                return 0;
            };
            if message.starts_with("fdbchat:") {
                // Chat commands can start networking and publish messages, so
                // only the chrome and our internal file pages may send them.
                if !SimpleHandler::is_ui_frame(&mut browser) {
                    return 0;
                }
                handle_chat_command(browser.identifier(), &message);
                return 1;
            }
            if !message.starts_with("fdb:") {
                return 0;
            }
            let action = {
                let mut inner = self.inner.lock().expect("Failed to lock inner");
                inner.bridge_action(&mut browser, &message)
            };
            match action {
                Some(BridgeAction::Navigate(target_id, url)) => spawn_navigation(target_id, url),
                Some(BridgeAction::Page(target_id, url)) => post_page(target_id, url),
                Some(BridgeAction::OpenSettings) => open_settings_overlay(),
                Some(BridgeAction::CloseSettings) => {
                    if let Some(instance) = SimpleHandler::instance() {
                        instance.lock().expect("Failed to lock handler").close_settings_view();
                    }
                }
                Some(BridgeAction::ToggleChatPanel) => toggle_chat_panel(),
                Some(BridgeAction::Review(domain)) => open_review_window_async(domain),
                Some(BridgeAction::About) => {
                    if let Some(instance) = SimpleHandler::instance() {
                        let version = env!("CARGO_PKG_VERSION");
                        instance
                            .lock()
                            .expect("Failed to lock handler")
                            .chrome_toast(&format!(
                                "Free Domain Browser CEF {version} · CEF 156.0.26 · GPL-3.0-only"
                            ));
                    }
                }
                Some(BridgeAction::CloseChatPanel) => {
                    if let Some(instance) = SimpleHandler::instance() {
                        instance.lock().expect("Failed to lock handler").close_chat_panel();
                    }
                }
                Some(BridgeAction::SaveSettings(settings)) => {
                    let _ = crate::settings::save(&settings);
                    if let Some(instance) = SimpleHandler::instance() {
                        instance
                            .lock()
                            .expect("Failed to lock handler")
                            .apply_settings();
                    }
                }
                Some(BridgeAction::BrowserCommand(target_id, kind)) => {
                    post_browser_command(target_id, kind);
                }
                Some(BridgeAction::Home(target_id)) => {
                    spawn_navigation(target_id, crate::settings::load().home);
                }
                Some(BridgeAction::External(url)) => {
                    if let Err(error) = open_external(&url) {
                        if let Some(instance) = SimpleHandler::instance() {
                            instance
                                .lock()
                                .expect("Failed to lock handler")
                                .chrome_toast(&format!("Could not open browser: {error}"));
                        }
                    }
                }
                Some(BridgeAction::ClearCache) => {
                    clear_registry_cache();
                    if let Some(instance) = SimpleHandler::instance() {
                        instance
                            .lock()
                            .expect("Failed to lock handler")
                            .chrome_toast("Registry cache cleared");
                    }
                }
                Some(BridgeAction::Recheck(target_id, domain)) => {
                    clear_registry_cache();
                    spawn_navigation(target_id, format!("fdn://{domain}"));
                }
                Some(BridgeAction::FocusChrome) => {
                    let mut inner = self.inner.lock().expect("Failed to lock inner");
                    inner.chrome_focus("window.fdbFocusUrl&&fdbFocusUrl()");
                }
                Some(BridgeAction::NewTab(uid, url)) => {
                    let url = if url.trim().is_empty() {
                        internal_page_url(NEWTAB_HTML)
                    } else {
                        url
                    };
                    open_tab(self.inner.clone(), uid, url);
                }
                Some(BridgeAction::CloseTab(uid)) => {
                    close_tab(self.inner.clone(), uid);
                }
                Some(BridgeAction::SelectTab(uid)) => {
                    let mut inner = self.inner.lock().expect("Failed to lock inner");
                    inner.switch_uid(uid);
                }
                Some(BridgeAction::Quit) => {
                    let mut inner = self.inner.lock().expect("Failed to lock inner");
                    inner.close_all_browsers(true);
                }
                None => {}
            }
            1
        }
    }
}

wrap_keyboard_handler! {
    struct SimpleHandlerKeyboardHandler {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: *mut u8,
            _is_keyboard_shortcut: Option<&mut i32>,
        ) -> i32 {
            let (Some(browser), Some(event)) = (browser, event) else {
                return 0;
            };
            if event.type_ == KeyEventType::KEYUP || event.type_ == KeyEventType::CHAR {
                return 0;
            }
            let key = event.windows_key_code;
            let modifiers = event.modifiers;
            let command = modifiers & (4 | 128) != 0;
            let alt = modifiers & 8 != 0;
            let shift = modifiers & 2 != 0;

            // The chrome runs its own keyboard handling and must keep it.
            {
                let inner = self.inner.lock().expect("Failed to lock inner");
                if inner.chrome_browser_id == Some(browser.identifier()) {
                    return 0;
                }
            }

            let script = if command && key == b'T' as i32 {
                Some(if shift {
                    "window.fdbRestoreTab&&fdbRestoreTab()"
                } else {
                    "window.fdbNewTab&&fdbNewTab()"
                })
            } else if command && key == b'W' as i32 {
                Some("window.fdbCloseActive&&fdbCloseActive()")
            } else if command && key == b'L' as i32 {
                // Focus the chrome view itself, not just the input element.
                if let Some(instance) = SimpleHandler::instance() {
                    if let Ok(inner) = instance.lock() {
                        inner.chrome_focus("window.fdbFocusUrl&&fdbFocusUrl()");
                    }
                }
                return 1;
            } else if command && key == b'R' as i32 {
                Some("window.fdbReload&&fdbReload()")
            } else if command && key == b'Q' as i32 {
                Some("window.fdbQuit&&fdbQuit()")
            } else if command && key == 188 {
                Some("window.fdbOpenSettings&&fdbOpenSettings()")
            } else if (alt && key == 37) || (command && key == 219) {
                Some("window.fdbBack&&fdbBack()")
            } else if (alt && key == 39) || (command && key == 221) {
                Some("window.fdbForward&&fdbForward()")
            } else if modifiers & 4 != 0 && key == 9 {
                Some(if shift {
                    "window.fdbCycleTab&&fdbCycleTab(-1)"
                } else {
                    "window.fdbCycleTab&&fdbCycleTab(1)"
                })
            } else if command && (49..=57).contains(&key) {
                // Handled below so the tab index can be interpolated.
                None
            } else if key == 116 {
                Some("window.fdbReload&&fdbReload()")
            } else {
                None
            };

            let script = match script {
                Some(script) => script.to_string(),
                None if command && (49..=57).contains(&key) => {
                    format!("window.fdbSelectTab&&fdbSelectTab({})", key - 49)
                }
                None => return 0,
            };
            if let Some(instance) = SimpleHandler::instance() {
                if let Ok(inner) = instance.lock() {
                    inner.chrome_execute(0, &script);
                }
            }
            1
        }
    }
}

wrap_life_span_handler! {
    struct SimpleHandlerLifeSpanHandler {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_after_created(browser);
        }

        fn do_close(&self, browser: Option<&mut Browser>) -> i32 {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.do_close(browser).into()
        }

        fn on_before_popup(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_before_popup(browser, target_url)
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_before_close(browser);
        }
    }
}

wrap_load_handler! {
    struct SimpleHandlerLoadHandler {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl LoadHandler {
        fn on_loading_state_change(
            &self,
            browser: Option<&mut Browser>,
            is_loading: i32,
            can_go_back: i32,
            can_go_forward: i32,
        ) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_loading_state_change(browser, is_loading != 0, can_go_back != 0, can_go_forward != 0);
        }

        fn on_load_start(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            transition_type: TransitionType,
        ) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_load_start(browser, frame, transition_type);
        }

        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: i32,
        ) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_load_end(browser, frame, http_status_code);
        }

        fn on_load_error(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_load_error(browser, frame, error_code, error_text, failed_url);
        }
    }
}

wrap_resource_request_handler! {
    struct SimpleHandlerResourceRequestHandler {
        initiator: Option<String>,
    }

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let settings = crate::settings::load();
            if let Some(request) = request {
                if settings.dnt {
                    let name = CefString::from("DNT");
                    let value = CefString::from("1");
                    request.set_header_by_name(Some(&name), Some(&value), 1);
                    let name = CefString::from("Sec-GPC");
                    request.set_header_by_name(Some(&name), Some(&value), 1);
                }
                if settings.cookies {
                    let initiator_host = self
                        .initiator
                        .as_deref()
                        .and_then(|value| url::Url::parse(value).ok())
                        .and_then(|value| value.host_str().map(str::to_string));
                    let request_host = url::Url::parse(&CefString::from(&request.url()).to_string())
                        .ok()
                        .and_then(|value| value.host_str().map(str::to_string));
                    if let (Some(initiator), Some(target)) = (initiator_host, request_host) {
                        if initiator != target {
                            let name = CefString::from("Cookie");
                            let empty = CefString::from("");
                            request.set_header_by_name(Some(&name), Some(&empty), 1);
                        }
                    }
                }
            }
            ReturnValue::CONTINUE
        }
    }
}

wrap_request_handler! {
    struct SimpleHandlerRequestHandler {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: i32,
            is_redirect: i32,
        ) -> i32 {
            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.on_before_browse(browser, frame, request, user_gesture, is_redirect)
        }

        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: i32,
            _is_download: i32,
            request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut i32>,
        ) -> Option<ResourceRequestHandler> {
            let settings = crate::settings::load();
            if settings.dnt || settings.cookies {
                Some(SimpleHandlerResourceRequestHandler::new(
                    request_initiator.map(CefString::to_string),
                ))
            } else {
                None
            }
        }
    }
}

fn open_review_window_async(domain: String) {
    let Some(instance) = SimpleHandler::instance() else { return; };
    let Some(inner) = instance.lock().ok().and_then(|handler| handler.weak_self.upgrade()) else {
        return;
    };
    let mut url = url::Url::parse(
        "https://free-domain-name.github.io/tauri-browser/stumble.html",
    )
    .expect("valid review URL");
    url.query_pairs_mut().append_pair("domain", &domain);
    if !open_review_window(inner, url.as_str()) {
        if let Ok(handler) = instance.lock() {
            handler.chrome_toast("Could not open review window");
        }
    }
}

fn parent_overlay_view(window: &Window, view: &BrowserView) {
    let mut child = View::from(view);
    window.add_child_view(Some(&mut child));
    child.set_visible(1);
}

fn open_settings_overlay() {
    let Some(instance) = SimpleHandler::instance() else { return; };
    let (inner, exists) = {
        let handler = instance.lock().expect("Failed to lock handler");
        (handler.weak_self.upgrade(), handler.settings_view.is_some())
    };
    if exists {
        instance.lock().expect("Failed to lock handler").show_settings_view();
        return;
    }
    let Some(inner) = inner else { return; };
    let url = settings_page_url(&crate::settings::load());
    let Some(view) = create_content_view(inner, &url) else {
        if let Ok(handler) = instance.lock() {
            handler.chrome_toast("Could not open settings");
        }
        return;
    };
    let window = instance
        .lock()
        .expect("Failed to lock handler")
        .window
        .clone();
    if let Some(window) = window {
        parent_overlay_view(&window, &view);
    }
    instance
        .lock()
        .expect("Failed to lock handler")
        .attach_settings_view(view);
}

fn toggle_chat_panel() {
    let Some(instance) = SimpleHandler::instance() else { return; };
    let (inner, exists, open) = {
        let handler = instance.lock().expect("Failed to lock handler");
        (
            handler.weak_self.upgrade(),
            handler.chat_panel.is_some(),
            handler.chat_open,
        )
    };
    if exists {
        let mut handler = instance.lock().expect("Failed to lock handler");
        if open {
            handler.close_chat_panel();
        } else {
            handler.show_chat_panel();
        }
        return;
    }
    let Some(inner) = inner else { return; };
    let url = format!("{}&panel=1", internal_page_url(CHAT_HTML));
    let Some(view) = create_content_view(inner, &url) else {
        if let Ok(handler) = instance.lock() {
            handler.chrome_toast("Could not open chat panel");
        }
        return;
    };
    let window = instance
        .lock()
        .expect("Failed to lock handler")
        .window
        .clone();
    if let Some(window) = window {
        parent_overlay_view(&window, &view);
    }
    instance
        .lock()
        .expect("Failed to lock handler")
        .attach_chat_panel(view);
}

fn handle_chat_command(browser_id: i32, message: &str) {
    let Some(payload) = message.strip_prefix("fdbchat:") else { return; };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else { return; };
    let Some(cmd) = value.get("cmd").and_then(|cmd| cmd.as_str()).map(str::to_string) else { return; };
    if cmd == "close_panel" {
        if let Some(instance) = SimpleHandler::instance() {
            instance
                .lock()
                .expect("Failed to lock handler")
                .close_chat_panel();
        }
        return;
    }
    let service = crate::chat::service().clone();
    // CEF has one chat panel per tab; key the service by the panel's browser
    // identifier so closing this tab does not leave another tab's room open.
    let tab_id = format!("cef-{browser_id}");
    crate::chat::runtime().spawn(async move {
        let result: Result<serde_json::Value, String> = match cmd.as_str() {
            "snapshot" => Ok(serde_json::json!(service.snapshot().await)),
            "enable" => service
                .set_enabled(tab_id.clone(), true)
                .await
                .map(|snapshot| serde_json::json!(snapshot))
                .map_err(|error| error.to_string()),
            "disable" => service
                .set_enabled(tab_id.clone(), false)
                .await
                .map(|snapshot| serde_json::json!(snapshot))
                .map_err(|error| error.to_string()),
            "send" => {
                let text = value.get("text").and_then(|text| text.as_str()).unwrap_or_default().to_string();
                service.send(text).await.map(|_| serde_json::Value::Null).map_err(|error| error.to_string())
            }
            "confirm_age" => service
                .confirm_age()
                .await
                .map(|snapshot| serde_json::json!(snapshot))
                .map_err(|error| error.to_string()),
            "set_nickname" => {
                let nickname = value.get("nickname").and_then(|nickname| nickname.as_str()).unwrap_or_default().to_string();
                service.set_nickname(nickname).await.map(|snapshot| serde_json::json!(snapshot)).map_err(|error| error.to_string())
            }
            "set_profile" => {
                let profile = crate::chat::settings::Profile {
                    description: value.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    gender: value.get("gender").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    age_band: value.get("age_band").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    country: value.get("country").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    city: value.get("city").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                };
                let show = value.get("show").and_then(|v| v.as_bool()).unwrap_or(false);
                service.set_profile(profile, show).await.map(|snapshot| serde_json::json!(snapshot)).map_err(|error| error.to_string())
            }
            "ignore" => {
                let sender = value.get("sender").and_then(|sender| sender.as_str()).unwrap_or_default().to_string();
                service.ignore(sender).await.map(|snapshot| serde_json::json!(snapshot)).map_err(|error| error.to_string())
            }
            "unignore" => {
                let sender = value.get("sender").and_then(|sender| sender.as_str()).unwrap_or_default().to_string();
                service.unignore(sender).await.map(|snapshot| serde_json::json!(snapshot)).map_err(|error| error.to_string())
            }
            "clear_local_data" => service
                .clear_local_data()
                .await
                .map(|snapshot| serde_json::json!(snapshot))
                .map_err(|error| error.to_string()),
            _ => Err("unknown chat command".into()),
        };
        let payload = match result {
            Ok(data) => serde_json::json!({ "ok": true, "data": data }),
            Err(error) => serde_json::json!({ "ok": false, "error": error }),
        };
        let script = format!("window.fdbChatResult&&fdbChatResult({payload})");
        let mut task = ChatResponse::new(browser_id, script);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

fn open_tab(inner: Arc<Mutex<SimpleHandler>>, uid: u64, url: String) {
    {
        let mut handler = inner.lock().expect("Failed to lock inner");
        if handler.tab(uid).is_some() {
            handler.switch_uid(uid);
            return;
        }
    }

    // Never hold the handler lock across view creation: CEF invokes
    // `on_after_created`, which locks the same mutex. Queue the uid first so
    // the lifecycle callback can associate the browser with this tab.
    {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_uids.push_back(uid);
    }
    let Some(view) = crate::shared::simple_app::create_content_view(inner.clone(), &url) else {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_uids.retain(|pending| *pending != uid);
        return;
    };
    let window = {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_views.push((uid, view.clone()));
        handler.window.clone()
    };
    if let Some(window) = window {
        // Parent the view before its browser exists. CEF creates the browser
        // once the view is in the view hierarchy, then calls `on_after_created`.
        let mut child = View::from(&view);
        window.add_child_view_at(Some(&mut child), 0);
        let bounds = SimpleHandler::tab_bounds(&window);
        child.set_bounds(Some(&bounds));
        child.set_visible(0);
    }
    let mut handler = inner.lock().expect("Failed to lock inner");
    if handler.tab(uid).is_none() {
        handler.register_tab(uid, view);
    }
    handler.pending_views.retain(|(pending, _)| *pending != uid);
    handler.pending_uids.retain(|pending| *pending != uid);
    if handler.tab(uid).is_some() {
        handler.switch_uid(uid);
    }
}

fn close_tab(inner: Arc<Mutex<SimpleHandler>>, uid: u64) {
    let (removed, window) = {
        let mut handler = inner.lock().expect("Failed to lock inner");
        let removed = handler.take_tab(uid);
        (removed, handler.window.clone())
    };
    let Some((view, browser)) = removed else {
        return;
    };
    // Closing a chat tab must release its per-tab room; otherwise the iroh
    // session would keep running with no visible panel.
    if let Some(browser) = browser.as_ref() {
        let tab_id = format!("cef-{}", browser.identifier());
        let service = crate::chat::service().clone();
        crate::chat::runtime().spawn(async move {
            let _ = service.set_enabled(tab_id, false).await;
        });
    }
    if let Some(window) = window {
        let mut view = View::from(&view);
        window.remove_child_view(Some(&mut view));
    }
    let mut handler = inner.lock().expect("Failed to lock inner");
    if let Some(uid) = handler.active_uid {
        handler.switch_uid(uid);
    }
}

fn clear_registry_cache() {
    crate::proxy::clear_cache();
}

/// Open an http(s) URL in the user's normal browser. Registry names and the
/// synthetic internal URLs are deliberately rejected.
fn open_external(raw: &str) -> Result<(), String> {
    // A registry page has no public URL of its own. Resolve its fdn:// URL to
    // the registry target (or public fallback) before handing it to the OS.
    if raw.starts_with("fdn://") {
        let raw = raw.to_string();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let url = match runtime {
                Ok(runtime) => runtime.block_on(async move {
                    let cache = std::env::temp_dir().join("fdn-registry");
                    let Ok(registry) = fdn_core::RegistryClient::new(cache) else {
                        return None;
                    };
                    let Ok(navigator) = fdn_core::Navigator::new() else {
                        return None;
                    };
                    let input = raw.trim_start_matches("fdn://");
                    let outcome = navigator.resolve(&registry, input).await.ok()?;
                    if outcome.source == fdn_core::Source::GithubRegistry {
                        outcome.registry_target
                    } else {
                        outcome.public_url.or(outcome.url)
                    }
                }),
                Err(_) => None,
            };
            if let Some(url) = url {
                let _ = open_external_http(&url);
            }
        });
        return Ok(());
    }
    open_external_http(raw)
}

fn open_external_http(raw: &str) -> Result<(), String> {
    let parsed = url::Url::parse(raw).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("external URLs must be http(s)".to_string());
    }
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    command
        .arg(parsed.as_str())
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn spawn_navigation(target_id: i32, target: String) {
    // `fdn://name/path` is a convenience alias. The resolver below owns the
    // registry/public decision, so strip the scheme and treat the rest as input.
    let target = target
        .strip_prefix("fdn://")
        .map(str::to_string)
        .unwrap_or(target);
    if let Some(choice) = remembered_choice(&target) {
        post_page(target_id, choice);
        return;
    }
    // Normal web URLs must not wait on registry + public-DNS probes. Only
    // free-namespace names (or bare input) need the resolver.
    if let Ok(parsed) = url::Url::parse(&target) {
        if matches!(parsed.scheme(), "http" | "https")
            && parsed.host_str().is_some_and(|host| host != "fdn" && !host.ends_with(".fdn"))
        {
            post_page(target_id, target);
            return;
        }
    }
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let url = match runtime {
            Ok(runtime) => runtime.block_on(async move {
                let cache = std::env::temp_dir().join("fdn-registry");
                let Ok(registry) = fdn_core::RegistryClient::new(cache) else {
                    return target;
                };
                let Ok(navigator) = fdn_core::Navigator::new() else {
                    return target;
                };
                match navigator.resolve(&registry, &target).await {
                    Ok(outcome) => match outcome.kind {
                        fdn_core::OutcomeKind::Conflict => internal_page_url(&conflict_page(&outcome)),
                        fdn_core::OutcomeKind::Unregistered => internal_page_url(&claim_page(&outcome)),
                        fdn_core::OutcomeKind::Unavailable => internal_page_url(&unavailable_page(&outcome)),
                        _ => {
                            let registry = if outcome.source == fdn_core::Source::GithubRegistry {
                                crate::proxy::proxy_url(&outcome.domain, &outcome.path)
                                    .or_else(|| outcome.registry_target.clone())
                            } else {
                                None
                            };
                            registry
                                .or(outcome.public_url)
                                .or(outcome.url)
                                .unwrap_or(target)
                        }
                    },
                    Err(_) => target,
                }
            }),
            Err(_) => target,
        };
        let mut task = ResolveNavigation::new(target_id, url);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

fn post_browser_command(target_id: i32, kind: BrowserCommandKind) {
    std::thread::spawn(move || {
        let mut task = RunBrowserCommand::new(target_id, kind);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

fn post_page(target_id: i32, url: String) {
    // Always post from another thread: this function may be called while the
    // handler mutex is held, and CEF can execute a posted task synchronously.
    std::thread::spawn(move || {
        let mut task = ResolveNavigation::new(target_id, url);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

wrap_task! {
    struct RunBrowserCommand {
        target_id: i32,
        kind: BrowserCommandKind,
    }

    impl Task {
        fn execute(&self) {
            let target = {
                let Some(inner) = SimpleHandler::instance() else { return; };
                let inner = inner.lock().expect("Failed to lock inner");
                inner
                    .browser_list
                    .iter()
                    .find(|browser| browser.identifier() == self.target_id)
                    .cloned()
            };
            let Some(mut target) = target else { return; };
            match self.kind {
                BrowserCommandKind::Back => target.go_back(),
                BrowserCommandKind::Forward => target.go_forward(),
                BrowserCommandKind::Reload => target.reload(),
            }
        }
    }
}

wrap_task! {
    struct ResolveNavigation {
        target_id: i32,
        url: String,
    }

    impl Task {
        fn execute(&self) {
            // Never hold the handler lock across load_url: CEF invokes
            // on_before_browse synchronously for some schemes and that handler
            // locks the same mutex.
            let target = {
                let Some(inner) = SimpleHandler::instance() else { return; };
                let inner = inner.lock().expect("Failed to lock inner");
                inner
                    .browser_list
                    .iter()
                    .find(|browser| browser.identifier() == self.target_id)
                    .cloned()
            };
            if let Some(mut target) = target {
                if let Some(frame) = target.main_frame() {
                    let mut frame = frame;
                    frame.load_url(Some(&CefString::from(self.url.as_str())));
                }
            }
        }
    }
}

wrap_task! {
    struct ChatResponse {
        browser_id: i32,
        script: String,
    }

    impl Task {
        fn execute(&self) {
            let target = {
                let Some(inner) = SimpleHandler::instance() else { return; };
                let inner = inner.lock().expect("Failed to lock inner");
                inner
                    .browser_list
                    .iter()
                    .find(|browser| browser.identifier() == self.browser_id)
                    .cloned()
            };
            let Some(mut target) = target else { return; };
            if let Some(frame) = target.main_frame() {
                let mut frame = frame;
                frame.execute_java_script(Some(&CefString::from(self.script.as_str())), None, 0);
            }
        }
    }
}

wrap_task! {
    struct ShowMainWindow {
        inner: Arc<Mutex<SimpleHandler>>,
    }

    impl Task {
        fn execute(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);

            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.show_main_window();
        }
    }
}

wrap_task! {
    struct CloseAllBrowsers {
        inner: Arc<Mutex<SimpleHandler>>,
        force_close: bool,
    }

    impl Task {
        fn execute(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);

            // Snapshot the browsers and release the handler lock before asking
            // CEF to close them: close_browser re-enters the life-span handler.
            let browsers = {
                let mut inner = self.inner.lock().expect("Failed to lock inner");
                inner.is_closing = true;
                inner.browser_list.clone()
            };
            for mut browser in browsers {
                if let Some(host) = browser.host() {
                    host.close_browser(self.force_close.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::{choice_target, error_page, navigation_key, navigation_target, open_external, unavailable_page};

    #[test]
    fn accepts_chrome_navigation() {
        assert_eq!(
            navigation_target("https://fdb.invalid/navigate?url=https%3A%2F%2Fexample.com%2F").as_deref(),
            Some("https://example.com/")
        );
    }

    #[test]
    fn rejects_other_urls() {
        assert_eq!(navigation_target("https://yahoo.com/"), None);
        assert_eq!(navigation_target("https://fdb.invalid/other?url=x"), None);
        assert_eq!(navigation_target("not a url"), None);
    }

    #[test]
    fn parses_conflict_choices() {
        let choice = choice_target(
            "https://fdb.invalid/choose?domain=foo&target=https%3A%2F%2Ffoo.example%2F",
        );
        assert_eq!(
            choice,
            Some(("foo".into(), "https://foo.example/".into()))
        );
        assert_eq!(
            choice_target("https://fdb.invalid/choose?domain=foo&target=javascript%3Aalert(1)"),
            None
        );
    }

    #[test]
    fn external_open_accepts_web_and_resolves_free_names() {
        assert!(open_external("fdn://hello.fdn").is_ok());
        assert!(open_external("file:///tmp/secret").is_err());
        assert!(open_external("javascript:alert(1)").is_err());
    }

    #[test]
    fn unavailable_page_offers_retry_and_conditional_public_fallback() {
        let mut outcome = fdn_core::NavigationOutcome {
            kind: fdn_core::OutcomeKind::Unavailable,
            domain: "hello.fdn".into(),
            path: "/".into(),
            source: fdn_core::Source::GithubRegistry,
            url: None,
            registry_url: None,
            public_url: Some("https://hello.fdn/".into()),
            registry_target: None,
            message: Some("offline".into()),
        };
        let page = unavailable_page(&outcome);
        assert!(page.contains("Retry registry"));
        assert!(!page.contains("Continue to public web"));
        outcome.domain = "example.com".into();
        outcome.public_url = Some("https://example.com/".into());
        let page = unavailable_page(&outcome);
        assert!(page.contains("Continue to public web"));
    }

    #[test]
    fn normalizes_free_name_keys() {
        assert_eq!(navigation_key("fdn://Foo.fdn/"), "foo");
        assert_eq!(navigation_key("https://foo.fdn/path"), "foo");
        assert_eq!(navigation_key("FOO"), "foo");
    }

    #[test]
    fn error_page_escapes_input() {
        let page = error_page("https://example.com/?a=1&b=2", "<bad>", -105);
        assert!(page.contains("a=1&amp;b=2"));
        assert!(page.contains("&lt;bad&gt;"));
        assert!(page.contains("-105"));
        assert!(page.contains("Retry"));
    }
}
