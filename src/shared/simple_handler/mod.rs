use cef::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::shared::simple_app::create_content_view;

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

/// The platform specific OS event the keyboard handler receives. macOS passes
/// a raw `NSEvent` pointer, Linux an `XEvent` and Windows a `MSG`.
#[cfg(target_os = "macos")]
type OsEvent<'a> = *mut u8;
#[cfg(target_os = "linux")]
type OsEvent<'a> = Option<&'a mut cef::sys::XEvent>;
#[cfg(target_os = "windows")]
type OsEvent<'a> = Option<&'a mut cef::sys::MSG>;

static SIMPLE_HANDLER_INSTANCE: OnceLock<Weak<Mutex<SimpleHandler>>> = OnceLock::new();

#[cfg(target_os = "windows")]
/// Runs a native menu command.
///
/// The Linux and Windows menu bars call this with the scripts of the chosen
/// entry: `script` runs in the chrome view and `page_script` in the page of
/// the active tab.
pub(crate) fn run_menu_command(script: &str, page_script: &str) {
    let Some(instance) = SimpleHandler::instance() else {
        return;
    };
    let handler = instance.lock().expect("Failed to lock handler");
    if !script.is_empty() {
        handler.chrome_execute(0, script);
    }
    if !page_script.is_empty() {
        handler.active_tab_execute(page_script);
    }
}

/// The scripts of the menu entry with the given Win32 command id.
///
/// The ids follow the order of the shared menu model: the first entry owns
/// `FIRST_COMMAND_ID`, the next one the id after it. Separators have no id.
#[cfg(target_os = "windows")]
pub(crate) fn menu_command(id: usize) -> Option<(&'static str, &'static str)> {
    let mut current = crate::menu_win32::FIRST_COMMAND_ID;
    for menu in crate::menu::MENUS {
        for entry in menu.entries {
            if entry.is_separator() {
                continue;
            }
            if current == id {
                return Some((entry.script, entry.page_script));
            }
            current += 1;
        }
    }
    None
}

/// Opens the menu window.
///
/// Linux and Windows use it: there CEF draws its own window and cannot host
/// a native menu bar, so the menu is shown in a window of its own. The native
/// menu bar of macOS does the same work.
fn open_menu_window() {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let Some(instance) = SimpleHandler::instance() else {
            return;
        };
        crate::shared::simple_app::create_menu_window(instance);
        eprintln!("[menu] the menu window is opened");
    }
}

enum BridgeAction {
    Navigate(i32, String),
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
    Review(String),
    About,
    FocusChrome,
    Quit,
    /// Open the menu window (Linux, where there is no native menu bar).
    MenuBar,
    OpenMenu(i32, i32),
    /// Run the menu entry with this id.
    MenuAction(usize),
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
    /// What the tab shows. The settings page is a tab of its own, so the
    /// handler has to know which tab it may close again.
    page: TabPage,
}

/// What a tab shows. Only the settings page is special: it is opened from
/// the chrome, closed from the page itself and reused when the gear is
/// pressed again.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TabPage {
    /// A web page or one of the internal pages (start page, error, ...).
    Web,
    /// The settings page.
    Settings,
}

/// The address the settings tab carries in the tab strip. The browser turns
/// it into the settings page; the chrome shows it like any other address.
const SETTINGS_TAB_URL: &str = "fdb://settings";

// Chrome page layout: tab strip (32) + toolbar (64). The status bar strip is
// gone, so the content view starts right below the address bar.
const CHROME_HEIGHT: i32 = 79;
const CHAT_PANEL_WIDTH: i32 = 390;

pub struct SimpleHandler {
    is_alloy_style: bool,
    browser_list: Vec<Browser>,
    chrome_browser_id: Option<i32>,
    chrome_view: Option<View>,
    chat_panel: Option<BrowserView>,
    chat_open: bool,
    chat_panel_uid: Option<u64>,
    chat_tabs: HashSet<u64>,
    tabs: Vec<Tab>,
    /// Horizontal container under the chrome bar holding the tab views plus
    /// the chat overlay. Tauri parity: the site keeps a flex slot LEFT of
    /// the chat strip, so the chat never covers the page.
    content_panel: Option<Panel>,
    /// True while the chat view is a child of `content_panel` (the tail slot
    /// of its box; tabs are always inserted before it).
    panel_chat_child: bool,
    pending_uids: VecDeque<(u64, TabPage)>,
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
/// Shared styles for every internal page (Material-flavored tokens +
/// components + the compact density). Materialized once next to the page
/// files and linked from them; inlined for the data:-origin chrome page.
pub(crate) const UI_KIT_CSS: &str = include_str!("../../../assets/ui-kit.css");
const APPEARANCE_JS_TEMPLATE: &str = r#"<script>(function(){try{var q=new URLSearchParams(location.search);var theme=q.get('theme')||'system';var density=q.get('density')||'comfortable';var dark=theme==='dark'||(theme!=='light'&&window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches);var root=document.documentElement;root.dataset.theme=dark?'dark':'light';root.dataset.density=density}catch(e){}})();</script>"#;
/// Every internal page links the shared kit itself, so a page also looks
/// right when it is opened on its own. The link is versioned with the
/// content of the kit, so a changed stylesheet is never served from CEF's
/// file:// cache after an upgrade.
const KIT_LINK: &str = r#"<link rel="stylesheet" href="ui-kit.css">"#;
fn versioned_kit_link() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    UI_KIT_CSS.hash(&mut hasher);
    format!(
        r#"<link rel="stylesheet" href="ui-kit.css?v={:x}">"#,
        hasher.finish()
    )
}

/// Live-update helper for pages already open (the settings tab, the chat
/// panel, the start page): re-applies the appearance attributes and the
/// user's custom CSS.
fn appearance_live_script(settings: &crate::settings::Settings) -> String {
    let payload = serde_json::json!({
        "theme": settings.theme,
        "density": settings.density,
        "compact_tabs": settings.compact_tabs,
        "custom_css": settings.custom_css,
    });
    format!(
        r#"(function(){{try{{var s={payload};var dark=s.theme==='dark'||(s.theme!=='light'&&window.matchMedia&&window.matchMedia('(prefers-color-scheme: dark)').matches);var root=document.documentElement;root.dataset.theme=dark?'dark':'light';root.dataset.density=s.density||'comfortable';var style=document.getElementById('fdb-appearance');if(!style){{style=document.createElement('style');style.id='fdb-appearance';document.head.appendChild(style)}}style.textContent=s.custom_css||'';}}catch(e){{}}}})();"#
    )
}

/// The page a new tab opens and the page the browser starts on when the user
/// has not chosen a home page of their own.
pub fn start_page_url() -> String {
    internal_page_url(NEWTAB_HTML)
}

/// Materialize an internal page on disk so it gets a real file:// origin and
/// can use localStorage for settings.
fn internal_page_url(html: &str) -> String {
    let dir = std::env::temp_dir().join("free-domain-browser-cef-ui");
    let _ = std::fs::create_dir_all(&dir);
    // Every internal page links the shared kit; materialize it once beside
    // them so the relative <link> resolves on the file:// origin.
    let kit = dir.join("ui-kit.css");
    if std::fs::read(&kit).ok().as_deref() != Some(UI_KIT_CSS.as_bytes()) {
        let _ = std::fs::write(&kit, UI_KIT_CSS);
    }
    use std::hash::{Hash, Hasher};
    let kit_link = versioned_kit_link();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut hasher);
    // The kit version is part of the page name: a changed stylesheet then
    // means a new page file, and no cache can hold the old look.
    kit_link.hash(&mut hasher);
    let path = dir.join(format!("{:x}.html", hasher.finish()));
    // Every internal page gets the same appearance bootstrap. The query string
    // carries theme/density from the Rust settings store, and the page's own
    // kit link is versioned so a changed stylesheet is never cached.
    let themed = match html.find("</head>") {
        Some(index) => {
            let boot = APPEARANCE_JS_TEMPLATE;
            let mut value = String::with_capacity(html.len() + boot.len());
            value.push_str(&html[..index]);
            value.push_str(boot);
            value.push_str(&html[index..]);
            value.replace(KIT_LINK, &kit_link)
        }
        None => html.to_string(),
    };
    let _ = std::fs::write(&path, &themed);
    let settings = crate::settings::load();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("theme", &settings.theme)
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
        .append_pair("density", &settings.density)
        .append_pair("theme", &settings.theme)
        .append_pair("custom_css", &settings.custom_css)
        // The marker that tells the handler "this tab shows the settings".
        // A tab that navigates away loses it and is an ordinary tab again.
        .append_pair("fdbpage", "settings")
        .finish();
    format!("{base}&{query}")
}

/// What a tab shows, read from the address of its page.
fn page_kind_for_url(url: &str) -> TabPage {
    if url.contains("fdbpage=settings") {
        TabPage::Settings
    } else {
        TabPage::Web
    }
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
    } else if let Ok(parsed) = url::Url::parse(&value)
        && matches!(parsed.scheme(), "http" | "https" | "fdn")
        && let Some(host) = parsed.host_str()
    {
        value = host.to_string();
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
    let register_url = format!("https://github.com/free-domain-name/registry/new/main?{query}");
    let free_namespace = outcome.domain == "fdn" || outcome.domain.ends_with(".fdn");
    let badge = if free_namespace {
        "Available"
    } else {
        "Unregistered"
    };
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
    let register_url_js = serde_json::to_string(&register_url).unwrap_or_else(|_| "\"\"".into());
    CLAIM_HTML
        .replace("{{domain}}", &html_escape(&outcome.domain))
        .replace("{{domain_js}}", &domain_js)
        .replace("{{register_url_js}}", &register_url_js)
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
        r#"<button id="public" class="secondary" type="button">Continue to public web</button>"#
            .to_string()
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
                chat_panel: None,
                chat_open: false,
                chat_panel_uid: None,
                chat_tabs: HashSet::new(),
                tabs: Vec::new(),
                content_panel: None,
                panel_chat_child: false,
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

        let browser = browser.cloned();
        // The window title stays static (the browser's own name); site titles
        // belong to the tab strip only, so every view feeds update_chrome.
        if let Some(browser) = browser.as_ref()
            && self.chrome_browser_id != Some(browser.identifier())
        {
            let title = title.map(CefString::to_string).unwrap_or_default();
            self.update_chrome(browser.identifier(), None, Some(&title));
        }
    }

    fn update_chrome(&mut self, source_id: i32, url: Option<&str>, title: Option<&str>) {
        let Some(chrome_id) = self.chrome_browser_id else {
            return;
        };
        if source_id == chrome_id {
            return;
        }
        // A tab that leaves the settings page is an ordinary tab again: the
        // handler must not close the wrong tab when the settings page asks
        // to be closed.
        if let Some(url) = url {
            let kind = page_kind_for_url(url);
            if let Some(uid) = self.tab_uid(source_id)
                && let Some(tab) = self.tabs.iter_mut().find(|tab| tab.uid == uid)
            {
                tab.page = kind;
            }
        }
        if url.is_some_and(|value| value.starts_with("file://") || value.starts_with("data:")) {
            return;
        }
        let Some(uid) = self.tab_uid(source_id) else {
            return;
        };
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else {
            return;
        };
        let Some(frame) = chrome.main_frame() else {
            return;
        };
        let url = url.map(|value| serde_json::to_string(value).unwrap_or_default());
        let title = title.map(|value| serde_json::to_string(value).unwrap_or_default());
        let script = match (url, title) {
            (Some(url), Some(title)) => format!("window.fdbUpdate&&fdbUpdate({url},{title},{uid})"),
            (Some(url), None) => format!("window.fdbUpdate&&fdbUpdate({url},null,{uid})"),
            (None, Some(title)) => format!("window.fdbUpdate&&fdbUpdate(null,{title},{uid})"),
            (None, None) => return,
        };
        let frame = frame;
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
            "theme": settings.theme,
            "custom_css": settings.custom_css,
        });
        let script = format!("window.fdbSettings&&fdbSettings({payload})");
        self.chrome_execute(0, &script);
        // Live appearance: the settings tab, the chat panel and tabs on
        // internal pages keep their creation-time theme otherwise (Tauri
        // applies appearance document-wide). Public web content is untouched.
        let appearance = appearance_live_script(&settings);
        let internal_prefix = std::env::temp_dir()
            .join("free-domain-browser-cef-ui")
            .to_string_lossy()
            .to_string();
        let Some(chrome_id) = self.chrome_browser_id else {
            return;
        };
        for browser in self.browser_list.iter() {
            if browser.identifier() == chrome_id {
                continue;
            }
            let Some(frame) = browser.main_frame() else {
                continue;
            };
            let url = CefString::from(&frame.url()).to_string();
            if !url.starts_with("file://") || !url.contains(&internal_prefix) {
                continue;
            }
            let mut frame = browser.main_frame();
            if let Some(frame) = frame.as_mut() {
                frame.execute_java_script(Some(&CefString::from(appearance.as_str())), None, 0);
            }
        }
    }

    pub(crate) fn chrome_execute(&self, source_id: i32, script: &str) {
        let Some(chrome_id) = self.chrome_browser_id else {
            return;
        };
        if source_id == chrome_id {
            return;
        }
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else {
            return;
        };
        let Some(frame) = chrome.main_frame() else {
            return;
        };
        let frame = frame;
        frame.execute_java_script(Some(&CefString::from(script)), None, 0);
    }

    /// Runs JavaScript in the page of the active tab.
    ///
    /// The native Edit menu uses this: the page lives in its own browser, so
    /// the editing commands cannot run in the chrome view.
    pub(crate) fn active_tab_execute(&self, script: &str) {
        let Some(uid) = self.active_uid else {
            return;
        };
        let Some(tab) = self.tab(uid) else {
            return;
        };
        let Some(browser) = tab.view.browser() else {
            return;
        };
        let Some(frame) = browser.main_frame() else {
            return;
        };
        let frame = frame;
        frame.execute_java_script(Some(&CefString::from(script)), None, 0);
    }

    pub(crate) fn chrome_focus(&self, script: &str) {
        let Some(chrome_id) = self.chrome_browser_id else {
            return;
        };
        let mut chrome = self
            .browser_list
            .iter()
            .find(|browser| browser.identifier() == chrome_id)
            .cloned();
        let Some(chrome) = chrome.as_mut() else {
            return;
        };
        if let Some(host) = chrome.host() {
            host.set_focus(1);
        }
        if let Some(frame) = chrome.main_frame() {
            let frame = frame;
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
            self.register_tab(uid as u64, view.clone(), TabPage::Web);
        }
        if self.active_uid.is_none() {
            self.active_uid = self.tabs.first().map(|tab| tab.uid);
        }
    }

    fn register_tab(&mut self, uid: u64, view: BrowserView, page: TabPage) {
        if self.tabs.iter().any(|tab| tab.uid == uid) {
            return;
        }
        // A freshly created tab view is not visible yet, so CEF has not
        // realized its browser at registration time. browser_id stays
        // unrealized (-1) until on_after_created reports the browser.
        let browser_id = view
            .browser()
            .map(|browser| browser.identifier())
            .unwrap_or(-1);
        self.tabs.push(Tab {
            uid,
            browser_id,
            view,
            page,
        });
    }

    /// The settings tab, if one is open. The tab the user is looking at
    /// wins: the page that asks to be closed is the one on screen.
    fn settings_tab_uid(&self) -> Option<u64> {
        if let Some(uid) = self.active_uid
            && self
                .tab(uid)
                .is_some_and(|tab| tab.page == TabPage::Settings)
        {
            return Some(uid);
        }
        self.tabs
            .iter()
            .find(|tab| tab.page == TabPage::Settings)
            .map(|tab| tab.uid)
    }

    pub fn set_window(&mut self, window: Window) {
        self.window = Some(window);
    }

    pub fn set_chrome_view(&mut self, view: BrowserView) {
        self.chrome_view = Some(View::from(&view));
    }

    /// The hbox container under the chrome bar: tabs left (flex), chat strip
    /// right. The window layout gives this panel flex 1; this store only
    /// parents children into it.
    pub fn set_content_panel(&mut self, panel: Panel) {
        self.content_panel = Some(panel);
    }

    /// Insert a tab view into the content panel, always BEFORE the trailing
    /// overlay children (the chat strip) so it stays the rightmost child.
    /// Falls back to no-op when the panel does not exist yet; the
    /// window-creation path parents the initial tabs itself.
    pub fn attach_tab_view(&mut self, view: &BrowserView) {
        if self.content_panel.is_none() {
            return;
        }
        // Mutating the panel (add/flex/layout) can run a synchronous layout
        // pass that would re-enter the handler lock. The panel mutation runs
        // inside the posted PanelAddChild task instead.
        let child_view = view.clone();
        let mut task = PanelAddChild::new(child_view, PanelChild::Tab);
        std::thread::spawn(move || {
            post_task(ThreadId::UI, Some(&mut task));
        });
    }

    /// Append an overlay view (the chat strip) as the last child of the
    /// content panel with the given kind, and lay the panel out.
    fn attach_overlay_view(&mut self, view: &BrowserView, kind: PanelChild) {
        if self.content_panel.is_none() {
            return;
        }
        match kind {
            PanelChild::Chat => self.panel_chat_child = true,
            PanelChild::Tab => {}
        }
        let child_view = view.clone();
        let mut task = PanelAddChild::new(child_view, kind);
        std::thread::spawn(move || {
            post_task(ThreadId::UI, Some(&mut task));
        });
    }

    /// Area of the content panel, relative to that panel. The panel spans the
    /// full window width below the chrome bar, so a window-derived estimate
    /// stays correct even before the first layout pass.
    fn panel_bounds(window: &Window) -> Rect {
        let bounds = window.bounds();
        Rect {
            x: 0,
            y: 0,
            width: bounds.width.max(1),
            height: (bounds.height - CHROME_HEIGHT).max(1),
        }
    }

    fn tab_bounds(window: &Window) -> Rect {
        // Tab views live inside the content panel; their bounds are panel
        // relative (the box layout normally owns them, this is only the
        // placeholder before the first layout pass).
        Self::panel_bounds(window)
    }

    pub fn resize_tabs(&mut self, _bounds: &Rect) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let bounds = Self::tab_bounds(&window);
        for tab in &self.tabs {
            let view = View::from(&tab.view);
            view.set_bounds(Some(&bounds));
        }
        self.layout_overlays(&window);
    }

    fn chat_bounds(window: &Window) -> Rect {
        let bounds = Self::panel_bounds(window);
        let width = CHAT_PANEL_WIDTH.min(bounds.width.max(1));
        Rect {
            x: (bounds.width - width).max(0),
            y: 0,
            width,
            height: bounds.height.max(1),
        }
    }

    fn layout_overlays(&self, window: &Window) {
        if let Some(view) = self.chat_panel.as_ref() {
            let view = View::from(view);
            view.set_bounds(Some(&Self::chat_bounds(window)));
        }
        if self.content_panel.is_some() {
            post_panel_layout();
        }
    }

    pub fn attach_chat_panel(&mut self, view: BrowserView) {
        let Some(window) = self.window.clone() else {
            return;
        };
        self.chat_panel = Some(view);
        self.chat_panel_uid = self.active_uid;
        if let Some(uid) = self.active_uid {
            self.chat_tabs.insert(uid);
        }
        self.chat_open = true;
        if let Some(chat) = self.chat_panel.clone() {
            self.attach_overlay_view(&chat, PanelChild::Chat);
        }
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(true)");
        if let Some(browser) = self.chat_panel.as_ref().and_then(|view| view.browser()) {
            if let Some(host) = browser.host() {
                host.set_focus(1);
            }
            if let Some(frame) = browser.main_frame() {
                let frame = frame;
                frame.execute_java_script(
                    Some(&CefString::from(
                        "window.fdbChatPanelOpen&&fdbChatPanelOpen()",
                    )),
                    None,
                    0,
                );
            }
        }
    }

    pub fn show_chat_panel(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if let Some(view) = self.chat_panel.as_ref() {
            let view = View::from(view);
            view.set_visible(1);
        }
        if self.chat_panel.is_some()
            && !self.panel_chat_child
            && let Some(chat) = self.chat_panel.clone()
        {
            self.attach_overlay_view(&chat, PanelChild::Chat);
        }
        self.chat_panel_uid = self.active_uid;
        if let Some(uid) = self.active_uid {
            self.chat_tabs.insert(uid);
        }
        self.chat_open = true;
        self.layout_overlays(&window);
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(true)");
        if let Some(browser) = self.chat_panel.as_ref().and_then(|view| view.browser())
            && let Some(host) = browser.host()
        {
            host.set_focus(1);
        }
    }

    /// Hide the panel without disabling its per-tab chat service. This mirrors
    /// the Tauri UI's syncChatToActiveTab(): the enabled state belongs to the
    /// tab, while the panel itself is only visible on an enabled tab.
    pub fn hide_chat_panel(&mut self) {
        if let Some(view) = self.chat_panel.as_ref() {
            let view = View::from(view);
            view.set_visible(0);
        }
        self.chat_open = false;
        self.chrome_execute(0, "window.fdbChatPanel&&fdbChatPanel(false)");
    }

    pub fn close_chat_panel(&mut self) {
        let uid = self.chat_panel_uid.or(self.active_uid);
        if let Some(uid) = uid {
            self.chat_tabs.remove(&uid);
        }
        self.chat_panel_uid = None;
        if let Some(view) = self.chat_panel.as_ref()
            && let Some(browser) = view.browser()
            && let Some(frame) = browser.main_frame()
        {
            let frame = frame;
            frame.execute_java_script(
                Some(&CefString::from("window.fdbPanelClose&&fdbPanelClose()")),
                None,
                0,
            );
        }
        self.hide_chat_panel();
    }

    /// Every view currently presenting the chat page: the chat panel itself
    /// plus any tab hosting chat.html. Chat event pushes fan out to all of
    /// them so background chat tabs stay fresh too.
    pub(crate) fn chat_page_browsers(&self) -> Vec<Browser> {
        let mut browsers = Vec::new();
        if let Some(view) = self.chat_panel.as_ref()
            && let Some(browser) = view.browser()
        {
            browsers.push(browser);
        }
        for uid in &self.chat_tabs {
            if let Some(tab) = self.tabs.iter().find(|tab| tab.uid == *uid)
                && let Some(browser) = tab.view.browser()
            {
                browsers.push(browser);
            }
        }
        browsers
    }

    fn chrome_state(&mut self) {
        self.chrome_execute(
            0,
            &format!("window.fdbChatPanel&&fdbChatPanel({})", self.chat_open),
        );
        // Full tab-strip resync: after an F5 reload of the chrome page its JS
        // state is gone; without this the strip resets to one placeholder tab.
        let tabs: Vec<serde_json::Value> = self
            .tabs
            .iter()
            .map(|tab| {
                // Tabs on internal pages (newtab/claim/conflict) have no public
                // URL; they sync as empty "New Tab" entries so every handler
                // tab still has a strip entry after the reload. The settings tab
                // keeps its own address, so the gear still finds it.
                let url = tab
                    .view
                    .browser()
                    .and_then(|b| {
                        b.main_frame()
                            .map(|frame| CefString::from(&frame.url()).to_string())
                    })
                    .unwrap_or_default();
                if tab.page == TabPage::Settings {
                    return serde_json::json!({
                        "uid": tab.uid,
                        "url": SETTINGS_TAB_URL,
                        "label": "Settings",
                    });
                }
                let display = if url.starts_with("file://") || url.starts_with("data:") {
                    String::new()
                } else {
                    crate::proxy::display_url(&url).unwrap_or(url)
                };
                serde_json::json!({"uid": tab.uid, "url": display})
            })
            .collect();
        let active = self
            .active_uid
            .map(|uid| uid.to_string())
            .unwrap_or_else(|| "null".to_string());
        if !tabs.is_empty() {
            let script = format!(
                "window.fdbTabs&&fdbTabs({},{})",
                serde_json::to_string(&tabs).unwrap_or_default(),
                active
            );
            self.chrome_execute(0, &script);
        }
    }

    fn switch_uid(&mut self, uid: u64) {
        if self.tab(uid).is_none() {
            return;
        }
        if self.chat_open && self.chat_panel_uid != Some(uid) {
            self.hide_chat_panel();
        }
        self.active_uid = Some(uid);
        for tab in &self.tabs {
            let view = View::from(&tab.view);
            view.set_visible(i32::from(tab.uid == uid));
        }
        // Tauri parity (activateTab): the selected page takes keyboard focus so
        // keystrokes land in the content, not the chrome strip — unless the
        // chat panel is about to take it.
        if !self.chat_open
            && let Some(tab) = self.tab(uid)
            && let Some(browser) = tab.view.browser()
            && let Some(host) = browser.host()
        {
            host.set_focus(1);
        }
        if !self.chat_open && self.chat_tabs.contains(&uid) {
            self.show_chat_panel();
        }
    }

    fn active_browser_id(&self) -> Option<i32> {
        let uid = self.active_uid?;
        self.tab(uid).map(|tab| tab.browser_id)
    }

    fn take_tab(&mut self, uid: u64) -> Option<(BrowserView, Option<Browser>)> {
        let index = self.tabs.iter().position(|tab| tab.uid == uid)?;
        let tab = self.tabs.remove(index);
        self.chat_tabs.remove(&uid);
        if self.chat_panel_uid == Some(uid) {
            self.chat_panel_uid = None;
        }
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
        let Some(browser) = browser else {
            return;
        };
        let source_id = browser.identifier();
        // The status bubble is global chrome chrome; a background tab's hover
        // status must not overwrite what the active tab is showing.
        if self.tab_uid(source_id) != self.active_uid {
            return;
        }
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
        let Some(browser) = browser else {
            return;
        };
        let source_id = browser.identifier();
        let Some(uid) = self.tab_uid(source_id) else {
            return;
        };
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
            // The chrome browser is realized with the window, so this is the
            // first moment the native menu bar can be attached to it.
            #[cfg(target_os = "windows")]
            install_menu_bar(Some(&mut browser));
            self.browser_list.push(browser);
            return;
        }

        self.browser_list.push(browser.clone());

        // Identify the fixed chrome view by identity, not by URL or creation
        // order: the URL is still empty during `on_after_created`.
        if let (Some(chrome), Some(view)) = (
            self.chrome_view.as_mut(),
            browser_view_get_for_browser(Some(&mut browser)),
        ) {
            let mut candidate = View::from(&view);
            if chrome.is_same(Some(&mut candidate)) != 0 {
                self.chrome_browser_id = Some(browser.identifier());
                // The chrome browser is realized with the window, so this is
                // the first moment the native menu bar can be attached to it.
                #[cfg(target_os = "windows")]
                install_menu_bar(Some(&mut browser));
                return;
            }
        }

        if let Some((uid, page)) = self.pending_uids.front().copied() {
            if let Some(position) = self
                .pending_views
                .iter()
                .position(|(pending, _)| *pending == uid)
            {
                let (_, view) = self.pending_views.remove(position);
                self.pending_uids.pop_front();
                self.register_tab(uid, view, page);
                self.switch_uid(uid);
            }
            return;
        }

        // A registered-but-not-yet-realized tab gets its browser_id here.
        if let Some(view) = browser_view_get_for_browser(Some(&mut browser)) {
            let mut as_view = View::from(&view);
            for tab in self.tabs.iter_mut() {
                if tab.browser_id < 0 {
                    let tab_view = View::from(&tab.view);
                    if tab_view.is_same(Some(&mut as_view)) != 0 {
                        tab.browser_id = browser.identifier();
                        break;
                    }
                }
            }
        }
        if let Some((uid, page)) = self.pending_uids.front().copied() {
            if let Some(position) = self
                .pending_views
                .iter()
                .position(|(pending, _)| *pending == uid)
            {
                let (_, view) = self.pending_views.remove(position);
                self.pending_uids.pop_front();
                self.register_tab(uid, view, page);
                self.switch_uid(uid);
            }
            return;
        }
        if self.tabs.is_empty()
            && let Some(view) = browser_view_get_for_browser(Some(&mut browser))
        {
            self.register_tab(0, view, TabPage::Web);
            self.active_uid = Some(0);
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
            if self.active_uid.is_some_and(|uid| self.tab(uid).is_none()) {
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
        let (Some(browser), Some(frame)) = (browser, frame) else {
            return;
        };
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
            self.register_tab(uid, view, TabPage::Web);
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
        let Some(browser) = browser else {
            return 0;
        };
        if self.chrome_browser_id == Some(browser.identifier()) {
            return 0;
        }
        let Some(target_url) = target_url else {
            return 1;
        };
        let url = target_url.to_string();
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("fdn://") {
            if let Some(instance) = SimpleHandler::instance() {
                let payload = serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".to_string());
                let script = format!("window.fdbNewTabUrl&&fdbNewTabUrl({payload})");
                if let Ok(inner) = instance.lock() {
                    inner.chrome_execute(0, &script);
                }
            }
            return 1;
        }
        // Tauri parity: every window.open is denied, never a new top-level
        // window. http(s)/fdn targets become tabs above; everything else
        // (about:blank, window.open(""), custom schemes) is simply blocked.
        // `devtools://` still falls through to the per-popup-view handling so
        // the debug workflow keeps working.
        if url.starts_with("devtools://") {
            return 0;
        }
        1
    }

    fn on_load_end(
        &mut self,
        browser: Option<&mut Browser>,
        _frame: Option<&mut Frame>,
        _http_status_code: i32,
    ) {
        let Some(browser) = browser else {
            return;
        };
        if self.chrome_browser_id != Some(browser.identifier()) {
            return;
        }
        self.apply_settings();
        // After an F5 reload of the chrome page the ⚙/💬 active states and the
        // tab strip must be re-synced (chrome_state was previously dead code).
        self.chrome_state();
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

        let Some(frame) = frame else {
            return;
        };
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
        let Some(request) = request else {
            return 0;
        };
        let url = CefString::from(&request.url()).to_string();
        let Some(current) = browser else {
            return 0;
        };
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
            post_page(target_id, settings_page_url(&crate::settings::load()));
            return 1;
        }
        if url == "https://fdb.invalid/chat" {
            post_page(target_id, internal_page_url(CHAT_HTML));
            return 1;
        }
        let Some(target) = navigation_target(&url) else {
            return 0;
        };
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
                    || (url.starts_with("file://") && url.contains("/free-domain-browser-cef-ui/"))
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
                } else if let Some(uid) = value.get("uid").and_then(|uid| uid.as_u64()) {
                    // Popup-originated navigations target the tab that was
                    // created for them, not whichever tab is active later.
                    self.tabs
                        .iter()
                        .find(|tab| tab.uid == uid)
                        .map(|tab| tab.browser_id)
                        .map(|target| BridgeAction::Navigate(target, url.to_string()))
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
            "about" => Some(BridgeAction::About),
            "menu_bar" => Some(BridgeAction::MenuBar),
            "menu_open" => {
                // The button reports where it is on screen so the native menu
                // opens right below it.
                let x = value.get("x").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                let y = value.get("y").and_then(|y| y.as_i64()).unwrap_or(0) as i32;
                Some(BridgeAction::OpenMenu(x, y))
            }
            "menu_action" => value
                .get("id")
                .and_then(|id| id.as_u64())
                .map(|id| BridgeAction::MenuAction(id as usize)),
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
            "back" => Some(BridgeAction::BrowserCommand(
                target_id,
                BrowserCommandKind::Back,
            )),
            "forward" => Some(BridgeAction::BrowserCommand(
                target_id,
                BrowserCommandKind::Forward,
            )),
            "reload" => Some(BridgeAction::BrowserCommand(
                target_id,
                BrowserCommandKind::Reload,
            )),
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
            if let Some(instance) = SimpleHandler::instance()
                && let Ok(inner) = instance.lock() {
                    inner.chrome_toast(&format!("Downloading to {}", path.display()));
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
            if let Some(instance) = SimpleHandler::instance()
                && let Ok(inner) = instance.lock() {
                    if done {
                        inner.chrome_toast("Download complete");
                    } else {
                        inner.chrome_toast(&format!("Downloading… {percent}%"));
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
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("download");
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
                Some(BridgeAction::OpenSettings) => {
                    // The settings page lives in a tab of its own. The chrome
                    // owns the tab strip, so it creates the tab and tells the
                    // handler which one it is.
                    let inner = self.inner.lock().expect("Failed to lock inner");
                    inner.chrome_execute(0, "window.fdbOpenSettings&&fdbOpenSettings()");
                }
                Some(BridgeAction::CloseSettings) => close_settings_tab(),
                Some(BridgeAction::ToggleChatPanel) => toggle_chat_panel(),
                Some(BridgeAction::Review(domain)) => open_review_in_system_browser(domain),
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
                    // Tauri parity: the home button shows the new-tab/start
                    // page. The `home` setting stays the startup URL only.
                    // The empty-URL update clears the address bar for the tab
                    // (the page itself is a file:// URL, which update_chrome
                    // skips), so the chrome reflects a fresh back-to-start tab.
                    post_page(target_id, internal_page_url(NEWTAB_HTML));
                    if let Some(instance) = SimpleHandler::instance()
                        && let Ok(mut handler) = instance.lock() {
                            handler.update_chrome(target_id, Some(""), None);
                        }
                }
                Some(BridgeAction::External(url)) => {
                    if let Err(error) = open_external(&url)
                        && let Some(instance) = SimpleHandler::instance() {
                            instance
                                .lock()
                                .expect("Failed to lock handler")
                                .chrome_toast(&format!("Could not open browser: {error}"));
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
                    let inner = self.inner.lock().expect("Failed to lock inner");
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
                Some(BridgeAction::MenuBar) => {
                    open_menu_window();
                }
                Some(BridgeAction::OpenMenu(x, y)) => {
                    // The menu model and the popup are UI thread only, and the
                    // popup runs its own message loop while it is open, so the
                    // handler lock must not be held here.
                    let Some(instance) = SimpleHandler::instance() else {
                        return 1;
                    };
                    let window = {
                        let handler = instance.lock().expect("Failed to lock handler");
                        handler.window.clone()
                    };
                    let Some(window) = window else {
                        return 1;
                    };
                    let mut task = ShowToolbarMenu::new(window, x, y);
                    std::thread::spawn(move || {
                        post_task(ThreadId::UI, Some(&mut task));
                    });
                }
                Some(BridgeAction::MenuAction(id)) => {
                    let Some(entry) = crate::menu::entry(id) else {
                        return 1;
                    };
                    let inner = self.inner.lock().expect("Failed to lock inner");
                    if !entry.script.is_empty() {
                        inner.chrome_execute(0, entry.script);
                    }
                    if !entry.page_script.is_empty() {
                        inner.active_tab_execute(entry.page_script);
                    }
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
            // The OS event type is platform specific: macOS hands over a raw
            // NSEvent pointer, Linux an XEvent and Windows a MSG.
            _os_event: OsEvent<'_>,
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
                if let Some(instance) = SimpleHandler::instance()
                    && let Ok(inner) = instance.lock() {
                        inner.chrome_focus("window.fdbFocusUrl&&fdbFocusUrl()");
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
            } else if modifiers == 0 && key == 116 {
                // Plain F5 only: with modifiers it belongs to the OS/macOS and
                // must not trigger a reload.
                Some("window.fdbReload&&fdbReload()")
            } else if modifiers == 0 && key == 121 {
                // F10 opens the menu, the way every other browser does.
                open_menu_window();
                return 1;
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
            if let Some(instance) = SimpleHandler::instance()
                && let Ok(inner) = instance.lock() {
                    inner.chrome_execute(0, &script);
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
                    if let (Some(initiator), Some(target)) = (initiator_host, request_host)
                        && initiator != target {
                            let name = CefString::from("Cookie");
                            let empty = CefString::from("");
                            request.set_header_by_name(Some(&name), Some(&empty), 1);
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

/// Upvote/downvote opens the community review page in the user's default
/// browser: the page embeds giscus and GitHub OAuth, which only work
/// reliably outside the app's own windows — mirrors the review buttons'
/// purpose without the embedded login flow.
fn open_review_in_system_browser(domain: String) {
    let mut url = url::Url::parse("https://free-domain-name.github.io/tauri-browser/stumble.html")
        .expect("valid review URL");
    url.query_pairs_mut().append_pair("domain", &domain);
    if let Err(error) = open_external_http(url.as_str()) {
        let script = format!(
            "window.fdbStatus&&fdbStatus({})",
            serde_json::to_string(&format!("Could not open reviews: {error}")).unwrap_or_default()
        );
        let mut task = ChromeScript::new(script);
        post_task(ThreadId::UI, Some(&mut task));
    }
}

/// Close the settings tab. The page asks for this (the Close button and
/// Escape); the tab strip lives in the chrome document, so the chrome closes
/// the tab there and sends the close command back.
fn close_settings_tab() {
    let Some(instance) = SimpleHandler::instance() else {
        return;
    };
    let uid = {
        let handler = instance.lock().expect("Failed to lock handler");
        handler.settings_tab_uid()
    };
    let Some(uid) = uid else {
        return;
    };
    let handler = instance.lock().expect("Failed to lock handler");
    handler.chrome_execute(0, &format!("window.fdbCloseTab&&fdbCloseTab({uid})"));
}

fn toggle_chat_panel() {
    let Some(instance) = SimpleHandler::instance() else {
        return;
    };
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
    let Some(inner) = inner else {
        return;
    };
    let url = format!("{}&panel=1", internal_page_url(CHAT_HTML));
    let Some(view) = create_content_view(inner, &url) else {
        if let Ok(handler) = instance.lock() {
            handler.chrome_toast("Could not open chat panel");
        }
        return;
    };
    let mut handler = instance.lock().expect("Failed to lock handler");
    // Parenting happens inside attach_chat_panel: the view becomes the
    // rightmost child of the content panel (Tauri-style chat strip).
    handler.attach_chat_panel(view);
}

/// Bridge chat events from the async service into the CEF UI. The chat
/// singleton starts with a no-op sink (it is created before the handler
/// exists), so install the real one once, on the first chat command. Events
/// arrive on the chat runtime threads; `CefPostTask` is safe from any thread.
fn install_chat_sink() {
    static INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if INSTALLED.set(()).is_err() {
        return;
    }
    let sink: crate::chat::EventCallback = Arc::new(|event: &str, body: String| {
        let mut task = ChatEventTask::new(event.to_string(), body);
        post_task(ThreadId::UI, Some(&mut task));
    });
    crate::chat::service().set_callback(sink);
}

fn handle_chat_command(browser_id: i32, message: &str) {
    install_chat_sink();
    let Some(payload) = message.strip_prefix("fdbchat:") else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return;
    };
    let Some(cmd) = value
        .get("cmd")
        .and_then(|cmd| cmd.as_str())
        .map(str::to_string)
    else {
        return;
    };
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
                let text = value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .unwrap_or_default()
                    .to_string();
                service
                    .send(text)
                    .await
                    .map(|_| serde_json::Value::Null)
                    .map_err(|error| error.to_string())
            }
            "confirm_age" => service
                .confirm_age()
                .await
                .map(|snapshot| serde_json::json!(snapshot))
                .map_err(|error| error.to_string()),
            "set_nickname" => {
                let nickname = value
                    .get("nickname")
                    .and_then(|nickname| nickname.as_str())
                    .unwrap_or_default()
                    .to_string();
                service
                    .set_nickname(nickname)
                    .await
                    .map(|snapshot| serde_json::json!(snapshot))
                    .map_err(|error| error.to_string())
            }
            "set_profile" => {
                let profile = crate::chat::settings::Profile {
                    description: value
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    gender: value
                        .get("gender")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    age_band: value
                        .get("age_band")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    country: value
                        .get("country")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    city: value
                        .get("city")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                };
                let show = value.get("show").and_then(|v| v.as_bool()).unwrap_or(false);
                service
                    .set_profile(profile, show)
                    .await
                    .map(|snapshot| serde_json::json!(snapshot))
                    .map_err(|error| error.to_string())
            }
            "ignore" => {
                let sender = value
                    .get("sender")
                    .and_then(|sender| sender.as_str())
                    .unwrap_or_default()
                    .to_string();
                service
                    .ignore(sender)
                    .await
                    .map(|snapshot| serde_json::json!(snapshot))
                    .map_err(|error| error.to_string())
            }
            "unignore" => {
                let sender = value
                    .get("sender")
                    .and_then(|sender| sender.as_str())
                    .unwrap_or_default()
                    .to_string();
                service
                    .unignore(sender)
                    .await
                    .map(|snapshot| serde_json::json!(snapshot))
                    .map_err(|error| error.to_string())
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

    // The settings tab carries its own address in the strip; the page behind
    // it is the internal settings page, built from the saved settings.
    let page = if url.trim() == SETTINGS_TAB_URL {
        TabPage::Settings
    } else {
        TabPage::Web
    };
    let url = match page {
        TabPage::Settings => settings_page_url(&crate::settings::load()),
        TabPage::Web => url,
    };

    // Never hold the handler lock across view creation: CEF invokes
    // `on_after_created`, which locks the same mutex. Queue the uid first so
    // the lifecycle callback can associate the browser with this tab.
    {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_uids.push_back((uid, page));
    }
    let Some(view) = crate::shared::simple_app::create_content_view(inner.clone(), &url) else {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_uids.retain(|(pending, _)| *pending != uid);
        return;
    };
    {
        let mut handler = inner.lock().expect("Failed to lock inner");
        handler.pending_views.push((uid, view.clone()));
        // Parent into the content panel (window-level parenting is done by
        // the app during creation; new tabs arrive here).
        handler.attach_tab_view(&view);
    }
    let mut handler = inner.lock().expect("Failed to lock inner");
    if handler.tab(uid).is_none() {
        handler.register_tab(uid, view, page);
    }
    handler.pending_views.retain(|(pending, _)| *pending != uid);
    handler.pending_uids.retain(|(pending, _)| *pending != uid);
    if handler.tab(uid).is_some() {
        handler.switch_uid(uid);
    }
}

fn close_tab(inner: Arc<Mutex<SimpleHandler>>, uid: u64) {
    let removed = {
        let mut handler = inner.lock().expect("Failed to lock inner");
        if handler.chat_panel_uid == Some(uid) {
            handler.close_chat_panel();
        }
        handler.take_tab(uid)
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
    // Tab views are children of the content panel now. Remove the view in a
    // posted UI task (panel mutations must run on the UI thread, unlocked).
    let mut task = PanelRemoveChild::new(View::from(&view));
    std::thread::spawn(move || {
        post_task(ThreadId::UI, Some(&mut task));
    });
    let mut handler = inner.lock().expect("Failed to lock inner");
    if let Some(uid) = handler.active_uid {
        handler.switch_uid(uid);
    }
    // Tauri parity: closing the last tab opens a fresh one instead of leaving
    // an empty page area behind. The tab strip lives in the chrome, so the new
    // tab must be created through the chrome's own JS hook.
    if handler.tabs.is_empty() && !handler.is_closing {
        handler.chrome_execute(0, "window.fdbNewTab&&fdbNewTab()");
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
        // The conflict page choice for this name is the deterministic target:
        // re-resolving here would ignore the user's pick (a Conflict outcome
        // carries no source, so the resolver falls back to the public mirror
        // even when the tab being viewed is the registry site).
        let name = raw.trim_start_matches("fdn://").to_string();
        let domain_key = name.split('/').next().unwrap_or("").to_string();
        let domain_key = domain_key.trim_end_matches(".fdn").to_string();
        if let Some(target) = remembered_choice(&domain_key)
            && let Ok(parsed) = url::Url::parse(&target)
            && matches!(parsed.scheme(), "http" | "https")
            && parsed.host_str().is_some_and(|host| host != "fdb.invalid")
        {
            return open_external_http(&target);
        }
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
            match url {
                Some(url) => {
                    let _ = open_external_http(&url);
                }
                None => {
                    // Nothing is openable for this name (unregistered or
                    // unavailable); surface it instead of failing silently.
                    let script = format!(
                        "window.fdbStatus&&fdbStatus({})",
                        serde_json::to_string(&format!("Nothing to open for {name}"))
                            .unwrap_or_default()
                    );
                    let mut task = ChromeScript::new(script);
                    post_task(ThreadId::UI, Some(&mut task));
                }
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
    // The settings address opens the settings page in the tab that asked for
    // it, the same way the gear opens it in a tab of its own.
    if target.trim() == SETTINGS_TAB_URL {
        post_page(target_id, settings_page_url(&crate::settings::load()));
        return;
    }
    // Navigation ordering guard (Tauri contentChain + per-tab generation): a
    // slow fdn resolve must not land AFTER a later, faster navigation. Every
    // navigation bumps the target's generation; a resolve that completes with
    // a stale generation is dropped instead of hijacking the page.
    let generation = bump_nav_generation(target_id);
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
    if let Ok(parsed) = url::Url::parse(&target)
        && matches!(parsed.scheme(), "http" | "https")
        && parsed
            .host_str()
            .is_some_and(|host| host != "fdn" && !host.ends_with(".fdn"))
    {
        post_page(target_id, target);
        return;
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
                        fdn_core::OutcomeKind::Conflict => {
                            internal_page_url(&conflict_page(&outcome))
                        }
                        fdn_core::OutcomeKind::Unregistered => {
                            internal_page_url(&claim_page(&outcome))
                        }
                        fdn_core::OutcomeKind::Unavailable => {
                            internal_page_url(&unavailable_page(&outcome))
                        }
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
        let mut task = ResolveNavigation::new(target_id, url, generation);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

/// Per-target navigation generation. Bumped by every navigation request so a
/// resolve started earlier can be dropped when the user navigates elsewhere
/// first (last click wins, matching the Tauri UI's per-tab generation).
fn nav_generations() -> &'static std::sync::Mutex<HashMap<i32, u64>> {
    static NAV_GEN: std::sync::OnceLock<std::sync::Mutex<HashMap<i32, u64>>> =
        std::sync::OnceLock::new();
    NAV_GEN.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn bump_nav_generation(target_id: i32) -> u64 {
    let mut map = nav_generations().lock().expect("nav gen lock");
    let entry = map.entry(target_id).or_insert(0);
    *entry += 1;
    *entry
}

fn nav_generation(target_id: i32) -> u64 {
    nav_generations()
        .lock()
        .ok()
        .and_then(|map| map.get(&target_id).copied())
        .unwrap_or(0)
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
    let generation = bump_nav_generation(target_id);
    std::thread::spawn(move || {
        let mut task = ResolveNavigation::new(target_id, url, generation);
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
            let Some(target) = target else { return; };
            match self.kind {
                BrowserCommandKind::Back => target.go_back(),
                BrowserCommandKind::Forward => target.go_forward(),
                BrowserCommandKind::Reload => target.reload(),
            }
        }
    }
}

// Runs a full layout pass of the content panel. Must execute AFTER the
// handler lock is released: CEF can re-enter `on_window_bounds_changed`,
// which locks the same mutex.
wrap_task! {
    struct PanelLayout;

    impl Task {
        fn execute(&self) {
            let panel = {
                let Some(instance) = SimpleHandler::instance() else { return; };
                let handler = instance.lock().expect("Failed to lock handler");
                handler.content_panel.clone()
            };
            let Some(panel) = panel else { return };
            panel.layout();
        }
    }
}

// Post a content-panel layout pass. Safe to call while the handler mutex is
// held: the post happens through a fresh thread the way the other UI tasks
// in this file do (a synchronous CefPostTask can run the task inline, which
// would deadlock against our own lock).

// What a content-panel child is. The kind is explicit on purpose: the chat
// strip shares its row with the tabs, so deriving "is a tab" from the flex
// value alone would misfile the strip as a tab and leave it hidden.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelChild {
    /// A tab page. Only the active tab is visible.
    Tab,
    /// The chat strip. A column on the right of the page.
    Chat,
}

// Add a view to the content panel, on the UI thread and without holding
// the handler lock. Every piece of context is re-read inside the task:
// wrappers created in this posted-task context are the live CEF objects
// (stored clones read from other threads report an empty view tree).
wrap_task! {
    struct PanelAddChild {
        view: BrowserView,
        kind: PanelChild,
    }

    impl Task {
        fn execute(&self) {
            let is_tab = self.kind == PanelChild::Tab;
            let (chrome_view, overlays, is_active_tab) = {
                let Some(instance) = SimpleHandler::instance() else { return };
                let handler = instance.lock().expect("Failed to lock handler");
                let chrome_view = handler.chrome_view.clone();
                let overlays = usize::from(handler.panel_chat_child);
                // switch_uid already made this tab active before the posted
                // task ran; a brand-new tab must attach VISIBLE, or the whole
                // panel paints empty when it and the previous tab are hidden.
                let is_active_tab = is_tab && handler.tabs.iter().any(|tab| {
                    tab.uid == handler.active_uid.unwrap_or(u64::MAX)
                        && View::from(&tab.view).is_same(Some(&mut View::from(&self.view))) != 0
                });
                (chrome_view, overlays, is_active_tab)
            };
            let Some(chrome_view) = chrome_view else { return };
            let Some(panel) = live_panel(&chrome_view) else { return };
            let panel_rect = panel.bounds();
            let mut child = View::from(&self.view);
            if is_tab {
                let index = panel.child_view_count().saturating_sub(overlays);
                panel.add_child_view_at(Some(&mut child), index as i32);
            } else {
                panel.add_child_view_at(Some(&mut child), panel.child_view_count() as i32);
            }
            // The chat strip is a right-hand column, and the tabs share the
            // remaining width.
            let flex = match self.kind {
                PanelChild::Tab => 1,
                PanelChild::Chat => 0,
            };
            if let Some(layout) =
                panel.get_layout().and_then(|layout| layout.as_box_layout())
            {
                layout.set_flex_for_view(Some(&mut child), flex);
            }
            let bounds = match self.kind {
                PanelChild::Chat => {
                    let width = CHAT_PANEL_WIDTH.min(panel_rect.width.max(1));
                    Rect {
                        x: (panel_rect.width - width).max(0),
                        y: 0,
                        width,
                        height: panel_rect.height.max(1),
                    }
                }
                PanelChild::Tab => panel_rect,
            };
            child.set_bounds(Some(&bounds));
            // Only a background tab may be invisible. The overlay is visible
            // as soon as it is attached; hiding the chat strip here is what
            // made the first click paint an empty panel.
            child.set_visible(if is_tab && !is_active_tab { 0 } else { 1 });
            panel.layout();
        }
    }
}

// Remove a view from the content panel, on the UI thread and unlocked
// (see PanelAddChild for the wrapper-staleness rationale).
wrap_task! {
    struct PanelRemoveChild {
        view: View,
    }

    impl Task {
        fn execute(&self) {
            let chrome_view = {
                let Some(instance) = SimpleHandler::instance() else { return };
                let handler = instance.lock().expect("Failed to lock handler");
                handler.chrome_view.clone()
            };
            let Some(chrome_view) = chrome_view else { return };
            let Some(panel) = live_panel(&chrome_view) else {
                return;
            };
            let mut view = self.view.clone();
            panel.remove_child_view(Some(&mut view));
            panel.layout();
        }
    }
}

/// Resolve the content panel from a live source. Stored Window/Panel clones
/// go stale after the event that created them (reads then report an empty
/// view tree), but BrowserView wrappers stay valid, so derive the panel
/// through the chrome bar's own window: [chrome, panel].
fn live_panel(chrome_view: &View) -> Option<Panel> {
    let view = chrome_view.clone();
    let window = view.window()?;
    let panel_child = window.child_view_at(1)?;
    panel_child.as_panel()
}

fn post_panel_layout() {
    std::thread::spawn(move || {
        let mut task = PanelLayout::new();
        post_task(ThreadId::UI, Some(&mut task));
    });
}

wrap_task! {
    struct ChromeScript {
        script: String,
    }

    impl Task {
        fn execute(&self) {
            let Some(inner) = SimpleHandler::instance() else { return; };
            let inner = inner.lock().expect("Failed to lock inner");
            inner.chrome_execute(0, &self.script);
        }
    }
}

// The native popup menu of the chrome bar's "☰" button.
//
// The chrome bar is a browser view of its own and only 79 pixels tall, so a
// dropdown drawn inside the chrome page is clipped away by the view. The
// button opens a native menu instead, the way other browsers do.
wrap_menu_model_delegate! {
    struct ToolbarMenuDelegate;

    impl MenuModelDelegate {
        fn execute_command(
            &self,
            _menu_model: Option<&mut MenuModel>,
            command_id: ::std::os::raw::c_int,
            _event_flags: EventFlags,
        ) {
            let Some(entry) = crate::menu::toolbar_entry(command_id) else {
                return;
            };
            run_toolbar_command(entry.script);
        }
    }
}

/// Runs a toolbar menu command in the chrome view.
fn run_toolbar_command(script: &str) {
    let Some(instance) = SimpleHandler::instance() else {
        return;
    };
    let handler = instance.lock().expect("Failed to lock inner");
    handler.chrome_execute(0, script);
}

// Shows the toolbar (☰) menu as a native popup menu.
//
// The menu model and the popup are UI thread only, and the popup runs its
// own message loop until the user picks an item: the handler lock must not be
// held while it is open.
wrap_task! {
    struct ShowToolbarMenu {
        window: Window,
        x: i32,
        y: i32,
    }

    impl Task {
        fn execute(&self) {
            // The button reports its position in the chrome page. The chrome
            // bar fills the top of the window, so the page origin is the
            // client area origin.
            let origin = self.window.client_area_bounds_in_screen();
            let point = Point {
                x: origin.x + self.x,
                y: origin.y + self.y,
            };
            let mut delegate = ToolbarMenuDelegate::new();
            let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
                return;
            };
            for (command_id, entry) in crate::menu::toolbar_entries() {
                match command_id {
                    Some(command_id) => {
                        model.add_item(command_id, Some(&CefString::from(entry.title)));
                    }
                    None => {
                        model.add_separator();
                    }
                }
            }
            self.window
                .show_menu(Some(&mut model), Some(&point), MenuAnchorPosition::TOPRIGHT);
        }
    }
}

wrap_task! {
    struct ResolveNavigation {
        target_id: i32,
        url: String,
        generation: u64,
    }

    impl Task {
        fn execute(&self) {
            // A newer navigation for this target already happened; this
            // resolve is stale and must not hijack the page (see
            // bump_nav_generation).
            if nav_generation(self.target_id) != self.generation {
                return;
            }
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
            if let Some(target) = target
                && let Some(frame) = target.main_frame() {
                    let frame = frame;
                    frame.load_url(Some(&CefString::from(self.url.as_str())));
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
            let Some(target) = target else { return; };
            if let Some(frame) = target.main_frame() {
                let frame = frame;
                frame.execute_java_script(Some(&CefString::from(self.script.as_str())), None, 0);
            }
        }
    }
}

wrap_task! {
    struct ChatEventTask {
        event: String,
        body: String,
    }

    impl Task {
        fn execute(&self) {
            // Pushes chat://state|message|presence|error into every view
            // hosting the chat page as an immediate `fdbChatEvent` script; the
            // page re-pulls a snapshot so the poll stays as a fallback only.
            let Some(instance) = SimpleHandler::instance() else { return; };
            let payload = serde_json::json!({ "kind": self.event, "data": self.body });
            let Ok(payload) = serde_json::to_string(&payload) else {
                return;
            };
            let script = format!("window.fdbChatEvent&&fdbChatEvent({payload})");
            let browsers = {
                let inner = instance.lock().expect("Failed to lock inner");
                inner.chat_page_browsers()
            };
            for browser in browsers {
                if let Some(frame) = browser.main_frame() {
                    frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
                }
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
            for browser in browsers {
                if let Some(host) = browser.host() {
                    host.close_browser(self.force_close.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::{
        choice_target, error_page, navigation_key, navigation_target, open_external,
        unavailable_page,
    };

    #[test]
    fn accepts_chrome_navigation() {
        assert_eq!(
            navigation_target("https://fdb.invalid/navigate?url=https%3A%2F%2Fexample.com%2F")
                .as_deref(),
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
        assert_eq!(choice, Some(("foo".into(), "https://foo.example/".into())));
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
