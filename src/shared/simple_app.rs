use cef::*;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock};

use super::simple_handler::*;

/// Height of the fixed HTML chrome bar, in DIP.
const CHROME_HEIGHT: i32 = 116;

wrap_window_delegate! {
    struct SimpleWindowDelegate {
        chrome_view: RefCell<Option<BrowserView>>,
        content_views: RefCell<Vec<BrowserView>>,
        runtime_style: RuntimeStyle,
        initial_show_state: ShowState,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size {
                width: 1200,
                height: 820,
            }
        }

        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            Size {
                width: 520,
                height: 420,
            }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            // Add the chrome bar and the page view. CEF clips child views, so the
            // page cannot paint over the chrome.
            let Some(window) = window else { return; };
            let chrome_view = self.chrome_view.borrow();
            let content_views = self.content_views.borrow();
            let mut chrome = chrome_view.as_ref().map(View::from);
            if let Some(browser) = chrome_view.as_ref().and_then(|view| view.browser()) {
                if let Some(handler) = SimpleHandler::instance() {
                    handler
                        .lock()
                        .expect("Failed to lock handler")
                        .mark_chrome_browser(browser.identifier());
                }
            }
            if let Some(handler) = SimpleHandler::instance() {
                let mut handler = handler.lock().expect("Failed to lock handler");
                handler.register_initial_tabs(&content_views);
                if let Some(chrome_view) = chrome_view.as_ref() {
                    handler.set_chrome_view(chrome_view.clone());
                }
                handler.set_window(window.clone());
            }
            // Add the chrome first so the vertical box layout places it above
            // the page. The page is still clipped to its own bounds below it.
            if let Some(chrome) = chrome.as_mut() {
                window.add_child_view(Some(chrome));
            }
            for (index, content) in content_views.iter().enumerate() {
                let mut content = View::from(content);
                window.add_child_view(Some(&mut content));
                content.set_visible(if index == 0 { 1 } else { 0 });
            }
            // A vertical box layout gives both BrowserViews a real layout pass.
            // Without it CEF can leave the chrome browser at the full window
            // height, hiding the page behind an opaque toolbar view.
            let settings = BoxLayoutSettings {
                horizontal: 0,
                default_flex: 0,
                ..Default::default()
            };
            if let Some(mut layout) = window.set_to_box_layout(Some(&settings)) {
                if let Some(first) = chrome.as_mut() {
                    layout.set_flex_for_view(Some(first), 0);
                }
                for content in content_views.iter() {
                    let mut content = View::from(content);
                    layout.set_flex_for_view(Some(&mut content), 1);
                }
                window.layout();
            }

            // Keep explicit bounds as well: they give the page view a concrete
            // drawable size before the first layout pass completes.
            let mut bounds = window.bounds();
            if bounds.width <= 1 || bounds.height <= 1 {
                bounds = Rect { x: 0, y: 0, width: 1200, height: 820 };
            }
            self.apply_bounds(chrome.as_mut(), &bounds);

            if self.initial_show_state != ShowState::HIDDEN {
                window.center_window(Some(&Size { width: 1200, height: 820 }));
                window.show();
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            *self.chrome_view.borrow_mut() = None;
            self.content_views.borrow_mut().clear();
        }

        fn on_window_bounds_changed(
            &self,
            _window: Option<&mut Window>,
            new_bounds: Option<&Rect>,
        ) {
            let Some(bounds) = new_bounds else { return; };
            let mut chrome = self.chrome_view.borrow().as_ref().map(View::from);
            self.apply_bounds(chrome.as_mut(), bounds);
            if let Some(handler) = SimpleHandler::instance() {
                handler
                    .lock()
                    .expect("Failed to lock handler")
                    .resize_tabs(bounds);
            }
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            // Allow the window to close if the browser says it's OK.
            let content_view = self.content_views.borrow();
            let browser_view = content_view.first().expect("BrowserView is None");
            if let Some(browser) = browser_view.browser() {
                let browser_host = browser.host().expect("BrowserHost is None");
                browser_host.try_close_browser()
            } else {
                1
            }
        }

        fn initial_show_state(&self, _window: Option<&mut Window>) -> ShowState {
            self.initial_show_state
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }

        // CEF hides the macOS traffic lights by default for Views windows.
        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 {
            1
        }
    }
}

impl SimpleWindowDelegate {
    fn apply_bounds(&self, chrome: Option<&mut View>, window_bounds: &Rect) {
        let width = window_bounds.width.max(1);
        let height = window_bounds.height.max(1);
        if let Some(chrome) = chrome {
            chrome.set_bounds(Some(&Rect {
                x: 0,
                y: 0,
                width,
                height: CHROME_HEIGHT.min(height),
            }));
        }
        let content_bounds = Rect {
            x: 0,
            y: CHROME_HEIGHT.min(height),
            width,
            height: (height - CHROME_HEIGHT).max(1),
        };
        for content in self.content_views.borrow().iter() {
            let mut content = View::from(content);
            content.set_bounds(Some(&content_bounds));
        }
    }
}

wrap_browser_view_delegate! {
    struct SimpleBrowserViewDelegate {
        runtime_style: RuntimeStyle,
        height: i32,
    }

    impl ViewDelegate {
        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 0, height: self.height }
        }
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 0, height: self.height }
        }
    }

    impl BrowserViewDelegate {
        fn on_popup_browser_view_created(
            &self,
            _browser_view: Option<&mut BrowserView>,
            popup_browser_view: Option<&mut BrowserView>,
            _is_devtools: i32,
        ) -> i32 {
            // Create a new top-level Window for the popup. It will show itself after
            // creation.
            let mut window_delegate = SimpleWindowDelegate::new(
                RefCell::new(None),
                RefCell::new(popup_browser_view.cloned().into_iter().collect()),
                self.runtime_style,
                ShowState::NORMAL,
            );
            window_create_top_level(Some(&mut window_delegate));

            // We created the Window.
            1
        }

        fn browser_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }
    }
}

/// Create a content BrowserView for a new tab. The caller must not hold the
/// handler mutex: `browser_view_create` synchronously invokes lifecycle
/// callbacks that lock it.
pub fn create_content_view(
    inner: std::sync::Arc<std::sync::Mutex<SimpleHandler>>,
    url: &str,
) -> Option<BrowserView> {
    let mut client = SimpleHandlerClient::new(inner);
    let settings = BrowserSettings::default();
    let url = CefString::from(url);
    let mut delegate = SimpleBrowserViewDelegate::new(RuntimeStyle::ALLOY, 0);
    browser_view_create(
        Some(&mut client),
        Some(&url),
        Some(&settings),
        None,
        None,
        Some(&mut delegate),
    )
}

wrap_app! {
    pub struct SimpleApp;

    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(SimpleBrowserProcessHandler::new(RefCell::new(None)))
        }
    }
}

wrap_browser_process_handler! {
    struct SimpleBrowserProcessHandler {
        client: RefCell<Option<Client>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            debug_assert_ne!(currently_on(ThreadId::UI), 0);

            // Check if Alloy style will be used.
            let command_line = command_line_get_global().expect("Failed to get command line");
            let use_alloy_style = true;
            let runtime_style = if use_alloy_style {
                RuntimeStyle::ALLOY
            } else {
                RuntimeStyle::DEFAULT
            };

            {
                // SimpleHandler implements browser-level callbacks.
                let mut client = self.client.borrow_mut();
                *client = Some(SimpleHandlerClient::new(SimpleHandler::new(
                    use_alloy_style,
                )));
            }

            // Specify CEF browser settings here.
            let settings = BrowserSettings::default();

            // Check if a "--url=" value was provided via the command-line. If so, use
            // that instead of the default URL.
            let url = CefString::from(&command_line.switch_value(Some(&CefString::from("url"))))
                .to_string();
            let url = if url.is_empty() {
                "https://yahoo.com/"
            } else {
                url.as_str()
            };
            let initial_page_url = url.to_string();
            let url = CefString::from(url);

            // Views is enabled by default (add `--use-native` to disable).
            let use_views = command_line.has_switch(Some(&CefString::from("use-native"))) == 0;

            // If using Views create the browser using the Views framework, otherwise
            // create the browser using the native platform framework.
            if use_views {
                // A fixed chrome bar, then Yahoo in a separate BrowserView
                // below it. CEF Views clips both views to their bounds.
                let initial_page_json = serde_json::to_string(&initial_page_url)
                    .unwrap_or_else(|_| "\"\"".to_string());
                let chrome_html = include_str!("../../assets/chrome.html")
                    .replace("__FDB_INITIAL_URL__", &initial_page_json);

                let chrome_url = get_data_uri(chrome_html.as_bytes(), "text/html");
                let chrome_url = CefString::from(chrome_url.as_str());
                let mut chrome_client = self.default_client();
                let mut chrome_delegate = SimpleBrowserViewDelegate::new(runtime_style, CHROME_HEIGHT);
                let chrome_view = browser_view_create(
                    chrome_client.as_mut(),
                    Some(&chrome_url),
                    Some(&settings),
                    None,
                    None,
                    Some(&mut chrome_delegate),
                );
                let mut client = self.default_client();
                let mut delegate = SimpleBrowserViewDelegate::new(runtime_style, 0);
                let browser_view = browser_view_create(
                    client.as_mut(),
                    Some(&url),
                    Some(&settings),
                    None,
                    None,
                    Some(&mut delegate),
                );
                // Optionally configure the initial show state.
                let initial_show_state = CefString::from(
                    &command_line.switch_value(Some(&CefString::from("initial-show-state"))),
                )
                .to_string();
                let initial_show_state = match initial_show_state.as_str() {
                    "minimized" => ShowState::MINIMIZED,
                    "maximized" => ShowState::MAXIMIZED,
                    // Hidden show state is only supported on MacOS.
                    #[cfg(target_os = "macos")]
                    "hidden" => ShowState::HIDDEN,
                    _ => ShowState::NORMAL,
                };

                // Create the Window. It will show itself after creation.
                let content_views: Vec<BrowserView> = browser_view.into_iter().collect();
                let mut delegate = SimpleWindowDelegate::new(
                    RefCell::new(chrome_view),
                    RefCell::new(content_views),
                    runtime_style,
                    initial_show_state,
                );
                window_create_top_level(Some(&mut delegate));
            } else {
                // Information used when creating the native window.
                let window_info = WindowInfo {
                    runtime_style,
                    ..Default::default()
                };

                #[cfg(target_os = "windows")]
                let window_info = window_info.set_as_popup(Default::default(), "cefsimple");

                let mut client = self.default_client();
                browser_host_create_browser(
                    Some(&window_info),
                    client.as_mut(),
                    Some(&url),
                    Some(&settings),
                    None,
                    None,
                );
            }
        }

        fn default_client(&self) -> Option<Client> {
            self.client.borrow().clone()
        }
    }
}

static REVIEW_SLOT: OnceLock<Arc<Mutex<Option<BrowserView>>>> = OnceLock::new();

fn review_slot() -> &'static Arc<Mutex<Option<BrowserView>>> {
    REVIEW_SLOT.get_or_init(|| Arc::new(Mutex::new(None)))
}

wrap_window_delegate! {
    struct ReviewWindowDelegate {
        view: RefCell<Option<BrowserView>>,
        slot: Arc<Mutex<Option<BrowserView>>>,
        runtime_style: RuntimeStyle,
        title: String,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 1040, height: 760 }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return; };
            let Some(view) = self.view.borrow().clone() else { return; };
            window.set_title(Some(&CefString::from(self.title.as_str())));
            let mut child = View::from(&view);
            window.add_child_view(Some(&mut child));
            child.set_visible(1);
            let bounds = window.bounds();
            child.set_bounds(Some(&Rect {
                x: 0,
                y: 0,
                width: bounds.width.max(1),
                height: bounds.height.max(1),
            }));
            window.show();
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            if let Ok(mut slot) = self.slot.lock() {
                *slot = None;
            }
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            self.runtime_style
        }

        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 {
            1
        }
    }
}

/// Open a Tauri-style dedicated review window instead of replacing a tab.
pub fn open_review_window(
    inner: std::sync::Arc<std::sync::Mutex<SimpleHandler>>,
    url: &str,
    domain: &str,
) -> bool {
    let slot = review_slot();
    if let Some(existing) = slot.lock().ok().and_then(|slot| slot.clone()) {
        if let Some(browser) = existing.browser() {
            if let Some(frame) = browser.main_frame() {
                let mut frame = frame;
                frame.load_url(Some(&CefString::from(url)));
                if let Some(window) = View::from(&existing).window() {
                    let title = format!("Review — {domain}");
                    window.set_title(Some(&CefString::from(title.as_str())));
                    window.show();
                    return true;
                }
            }
        }
        if let Ok(mut slot) = slot.lock() {
            *slot = None;
        }
    }
    let Some(view) = create_content_view(inner, url) else {
        return false;
    };
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(view.clone());
    }
    let mut delegate = ReviewWindowDelegate::new(
        RefCell::new(Some(view)),
        slot.clone(),
        RuntimeStyle::ALLOY,
        format!("Review — {domain}"),
    );
    window_create_top_level(Some(&mut delegate));
    true
}
