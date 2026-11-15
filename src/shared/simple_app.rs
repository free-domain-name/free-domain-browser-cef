use cef::*;
use std::cell::RefCell;


use super::simple_handler::*;

/// Height of the fixed HTML chrome bar, in DIP.
// Chrome page layout: tab strip (32) + toolbar (64) — no status-bar strip.
const CHROME_HEIGHT: i32 = 79;

wrap_window_delegate! {
    struct SimpleWindowDelegate {
        chrome_view: RefCell<Option<BrowserView>>,
        content_views: RefCell<Vec<BrowserView>>,
        content_panel: RefCell<Option<Panel>>,
        panel_delegate: RefCell<Option<PanelDelegate>>,
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
            // The OS window keeps the browser's own name; site titles only
            // appear in the chrome's tab strip.
            window.set_title(Some(&CefString::from("Free Domain Browser")));
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

            // Content structure, Tauri parity:
            //   window (vertical box) = [ chrome bar (flex 0), content panel (flex 1) ]
            //   content panel (horizontal box) = [ active tab (flex 1), chat strip (flex 0), settings (flex 1) ]
            // The site keeps its flex slot left of the chat, so the chat must
            // never cover the page. Popup windows (review) have no chrome bar
            // and keep the simple window-children path.
            let is_main_window = chrome.is_some();

            // Add the chrome first so the vertical box layout places it above
            // the content panel. The page is still clipped below the bar.
            if let Some(chrome) = chrome.as_mut() {
                window.add_child_view(Some(chrome));
            }

            let mut content_panel: Option<Panel> = None;
            let mut panel_delegate_holder: Option<PanelDelegate> = None;
            if is_main_window {
                // The delegate wrapper must outlive this function: keep it in
                // `panel_delegate`; cef-rs does not add a reference for us
                // here, and a dropped delegate turns the panel into a zombie
                // (adds and child counts silently stop working).
                let mut panel_delegate = SimplePanelDelegate::new();
                let panel = panel_create(Some(&mut panel_delegate));
                panel_delegate_holder = Some(panel_delegate);
                if let Some(panel) = panel.as_ref() {
                    if let Some(handler) = SimpleHandler::instance() {
                        handler
                            .lock()
                            .expect("Failed to lock handler")
                            .set_content_panel(panel.clone());
                    }
                }
                if let Some(panel) = panel.as_ref() {
                    // Parent the panel first, THEN install its box layout:
                    // the layout manager only survives if the panel is part
                    // of a window-rooted tree when it is set.
                    let mut panel_child = View::from(panel);
                    window.add_child_view(Some(&mut panel_child));
                    let panel_settings = BoxLayoutSettings {
                        horizontal: 1,
                        between_child_spacing: 0,
                        ..Default::default()
                    };
                    let mut panel = panel.clone();
                    panel.set_to_box_layout(Some(&panel_settings));
                    // BoxLayout::SetFlexForView silently ignores views that
                    // are not children yet — set flex AFTER add_child_view.
                    for (index, content) in content_views.iter().enumerate() {
                        let mut content = View::from(content);
                        panel.add_child_view(Some(&mut content));
                        if let Some(mut layout) =
                            panel.get_layout().and_then(|layout| layout.as_box_layout())
                        {
                            layout.set_flex_for_view(Some(&mut content), 1);
                        }
                        content.set_visible(if index == 0 { 1 } else { 0 });
                    }
                    panel.layout();
                }
                if chrome.is_some() {
                    *self.content_panel.borrow_mut() = panel.clone();
                }
                content_panel = panel;
            } else {
                for (index, content) in content_views.iter().enumerate() {
                    let mut content = View::from(content);
                    window.add_child_view(Some(&mut content));
                    content.set_visible(if index == 0 { 1 } else { 0 });
                }
            }
            *self.panel_delegate.borrow_mut() = panel_delegate_holder;

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
                if let Some(panel) = content_panel.as_ref() {
                    let mut panel_child = View::from(panel);
                    layout.set_flex_for_view(Some(&mut panel_child), 1);
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
            *self.content_panel.borrow_mut() = None;
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
        // The content panel gets the full area below the chrome bar; its own
        // horizontal box layout arranges the tab views inside it. Tab views
        // also get explicit panel-relative placeholder bounds: with a hidden
        // window no layout pass ever runs, and without them the site would
        // stay at zero size.
        let panel = self.content_panel.borrow();
        if let Some(panel) = panel.as_ref() {
            let mut panel_view = View::from(panel);
            panel_view.set_bounds(Some(&Rect {
                x: 0,
                y: CHROME_HEIGHT.min(height),
                width,
                height: (height - CHROME_HEIGHT).max(1),
            }));
            let tab_bounds = Rect {
                x: 0,
                y: 0,
                width,
                height: (height - CHROME_HEIGHT).max(1),
            };
            for content in self.content_views.borrow().iter() {
                let mut content = View::from(content);
                content.set_bounds(Some(&tab_bounds));
            }
        }
    }
}

wrap_panel_delegate! {
    // A plain container: its own preferred size is irrelevant because the
    // window box gives the panel a flex slot.
    struct SimplePanelDelegate {}

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 0, height: 0 }
        }

        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 0, height: 0 }
        }
    }

    impl PanelDelegate {}
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
                RefCell::new(None),
                RefCell::new(None),
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
                    .replace("__FDB_INITIAL_URL__", &initial_page_json)
                    // data: origins cannot resolve a relative stylesheet;
                    // inline the shared kit for the chrome page only.
                    .replace("</head>", &format!("<style>{}</style></head>", UI_KIT_CSS));

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
                    RefCell::new(None),
                    RefCell::new(None),
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


