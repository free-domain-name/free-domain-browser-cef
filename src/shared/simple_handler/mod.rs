use cef::*;
use std::sync::{Arc, Mutex, OnceLock, Weak};

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

#[cfg(not(target_os = "macos"))]
fn platform_show_window(_browser: Option<&mut Browser>) {
    todo!("Implement platform_show_window for non-macOS platforms");
}

static SIMPLE_HANDLER_INSTANCE: OnceLock<Weak<Mutex<SimpleHandler>>> = OnceLock::new();

enum BridgeAction {
    Navigate(i32, String),
    Page(i32, String),
    SaveSettings(crate::settings::Settings),
    BrowserCommand(i32, BrowserCommandKind),
    Home(i32),
}

#[derive(Clone, Copy)]
enum BrowserCommandKind {
    Back,
    Forward,
    Reload,
}

pub struct SimpleHandler {
    is_alloy_style: bool,
    browser_list: Vec<Browser>,
    chrome_browser_id: Option<i32>,
    is_closing: bool,
    weak_self: Weak<Mutex<Self>>,
}

const SETTINGS_HTML: &str = include_str!("../../../assets/settings.html");
const CHAT_HTML: &str = include_str!("../../../assets/chat.html");
const CONFLICT_HTML: &str = include_str!("../../../assets/conflict.html");
const CLAIM_HTML: &str = include_str!("../../../assets/claim.html");
const NEWTAB_HTML: &str = include_str!("../../../assets/newtab.html");

/// Materialize an internal page on disk so it gets a real file:// origin and
/// can use localStorage for settings.
fn internal_page_url(html: &str) -> String {
    let dir = std::env::temp_dir().join("free-domain-browser-cef-ui");
    let _ = std::fs::create_dir_all(&dir);
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut hasher);
    let path = dir.join(format!("{:x}.html", hasher.finish()));
    let _ = std::fs::write(&path, html);
    url::Url::from_file_path(&path)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| get_data_uri(html.as_bytes(), "text/html"))
}

fn settings_page_url(settings: &crate::settings::Settings) -> String {
    let base = internal_page_url(SETTINGS_HTML);
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("home", &settings.home)
        .append_pair("cookies", if settings.cookies { "1" } else { "0" })
        .append_pair("dnt", if settings.dnt { "1" } else { "0" })
        .finish();
    format!("{base}?{query}")
}

fn choice_url(target: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("target", target)
        .finish();
    format!("https://fdb.invalid/choose?{query}")
}

fn conflict_page(outcome: &fdn_core::NavigationOutcome) -> String {
    let target = outcome.registry_target.clone().unwrap_or_default();
    let public = outcome.public_url.clone().unwrap_or_default();
    CONFLICT_HTML
        .replace("{{domain}}", &outcome.domain)
        .replace("{{registry_choice}}", &choice_url(&target))
        .replace("{{public_choice}}", &choice_url(&public))
        .replace("{{target}}", &target)
}

fn claim_page(outcome: &fdn_core::NavigationOutcome) -> String {
    CLAIM_HTML
        .replace("{{domain}}", &outcome.domain)
        .replace(
            "{{message}}",
            outcome.message.as_deref().unwrap_or("This name is not registered yet."),
        )
        .replace(
            "{{public_url}}",
            outcome.public_url.as_deref().unwrap_or("https://github.com/free-domain-name/registry"),
        )
}

fn choice_target(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if parsed.host_str() != Some("fdb.invalid") || parsed.path() != "/choose" {
        return None;
    }
    parsed
        .query_pairs()
        .find(|(key, _)| key == "target")
        .map(|(_, value)| value.into_owned())
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
                is_closing: false,
                weak_self: weak.clone(),
            })
        })
    }

    fn on_title_change(&mut self, browser: Option<&mut Browser>, title: Option<&CefString>) {
        debug_assert_ne!(currently_on(ThreadId::UI), 0);

        let mut browser = browser.cloned();
        if let Some(browser_view) = browser_view_get_for_browser(browser.as_mut()) {
            if let Some(window) = browser_view.window() {
                window.set_title(title);
            }
        } else if self.is_alloy_style {
            platform_title_change(browser.as_mut(), title);
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
            (Some(url), Some(title)) => format!("window.fdbUpdate&&fdbUpdate({url},{title})"),
            (Some(url), None) => format!("window.fdbUpdate&&fdbUpdate({url},null)"),
            (None, Some(title)) => format!("window.fdbUpdate&&fdbUpdate(null,{title})"),
            (None, None) => return,
        };
        let mut frame = frame;
        frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
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

        // The chrome view is created first; remember it so page views cannot
        // drive chrome-only synthetic navigation.
        if self.chrome_browser_id.is_none() {
            self.chrome_browser_id = Some(browser.identifier());
        }

        // Add to the list of existing browsers.
        self.browser_list.push(browser);
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
        if let Some(index) = self
            .browser_list
            .iter()
            .position(move |elem| elem.is_same(Some(&mut browser)) != 0)
        {
            self.browser_list.remove(index);
        }

        if self.browser_list.is_empty() {
            // All browser windows have closed. Quit the application message loop.
            quit_message_loop();
        }
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

        let frame = frame.expect("Frame is None");

        // Display a load error message using a data: URI.
        let error_text = error_text.map(CefString::to_string).unwrap_or_default();
        let failed_url = failed_url.map(CefString::to_string).unwrap_or_default();
        let data = format!(
            r#"
            <html>
                <body bgcolor="white">
                    <h2>Failed to load URL {failed_url} with error {error_text} ({error_code}).</h2>
                </body>
            </html>
            "#
        );

        let uri = get_data_uri(data.as_bytes(), "text/html");
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
            if let Some(target) = choice_target(&url) {
                post_page(current_id, target);
                return 1;
            }
            return 0;
        }
        let Some(target_id) = self.other_browser_id(&mut current) else {
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

    fn other_browser_id(&mut self, current: &mut Browser) -> Option<i32> {
        self.browser_list
            .iter_mut()
            .find(|candidate| candidate.is_same(Some(current)) == 0)
            .map(|browser| browser.identifier())
    }



    /// Parse a chrome command without executing it. The caller must drop the
    /// handler lock before loading a page, because CEF re-enters navigation
    /// handlers synchronously.
    fn bridge_action(&mut self, browser: &mut Browser, message: &str) -> Option<BridgeAction> {
        if self.chrome_browser_id != Some(browser.identifier()) {
            return None;
        }
        let payload = message.strip_prefix("fdb:")?;
        let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
        let target_id = self.other_browser_id(browser)?;
        match value.get("cmd").and_then(|cmd| cmd.as_str())? {
            "navigate" => {
                let url = value.get("url")?.as_str()?.trim();
                if url.is_empty() {
                    None
                } else {
                    Some(BridgeAction::Navigate(target_id, url.to_string()))
                }
            }
            "settings" => Some(BridgeAction::Page(
                target_id,
                settings_page_url(&crate::settings::load()),
            )),
            "save_settings" => {
                crate::settings::Settings::from_value(&value).map(BridgeAction::SaveSettings)
            }
            "chat" => Some(BridgeAction::Page(target_id, internal_page_url(CHAT_HTML))),
            "new_tab" => Some(BridgeAction::Page(target_id, internal_page_url(NEWTAB_HTML))),
            "back" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Back)),
            "forward" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Forward)),
            "reload" => Some(BridgeAction::BrowserCommand(target_id, BrowserCommandKind::Reload)),
            "home" => Some(BridgeAction::Home(target_id)),
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
        if currently_on(thread_id) == 0 {
            // Execute on the UI thread.
            let this = self
                .weak_self
                .upgrade()
                .expect("Weak reference to SimpleHandler is None");
            let mut task = CloseAllBrowsers::new(this, force_close);
            post_task(thread_id, Some(&mut task));
            return;
        }

        for browser in self.browser_list.iter() {
            let browser_host = browser.host().expect("BrowserHost is None");
            browser_host.close_browser(force_close.into());
        }
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
    }
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

        fn on_address_change(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            let (Some(browser), Some(url)) = (browser, url) else { return; };
            let id = browser.identifier();
            let url = url.to_string();
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
                Some(BridgeAction::SaveSettings(settings)) => {
                    let _ = crate::settings::save(&settings);
                }
                Some(BridgeAction::BrowserCommand(target_id, kind)) => {
                    post_browser_command(target_id, kind);
                }
                Some(BridgeAction::Home(target_id)) => {
                    spawn_navigation(target_id, crate::settings::load().home);
                }
                None => {}
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
    }
}

fn spawn_navigation(target_id: i32, target: String) {
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
                        _ => outcome
                            .registry_target
                            .or(outcome.public_url)
                            .or(outcome.url)
                            .unwrap_or(target),
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

            let mut inner = self.inner.lock().expect("Failed to lock inner");
            inner.close_all_browsers(self.force_close);
        }
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::navigation_target;

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
}
