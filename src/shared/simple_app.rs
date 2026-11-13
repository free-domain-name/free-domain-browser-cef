use cef::*;
use std::cell::RefCell;

use super::simple_handler::*;

wrap_window_delegate! {
    struct SimpleWindowDelegate {
        chrome_view: RefCell<Option<BrowserView>>,
        content_view: RefCell<Option<BrowserView>>,
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
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            // Add the chrome bar and the page view. CEF clips child views, so the
            // page cannot paint over the chrome.
            let Some(window) = window else { return; };
            let chrome_view = self.chrome_view.borrow();
            let content_view = self.content_view.borrow();
            let mut chrome = chrome_view.as_ref().map(View::from);
            let mut content = content_view.as_ref().map(View::from);
            if let Some(chrome) = chrome.as_mut() {
                // chrome view has no explicit minimum-size API; fixed content bounds are used below.
                window.add_child_view(Some(chrome));
            }
            if let Some(content) = content.as_mut() {
                window.add_child_view(Some(content));
            }
            let settings = BoxLayoutSettings { horizontal: 0, ..Default::default() };
            if let Some(layout) = window.set_to_box_layout(Some(&settings)) {
                if let Some(chrome) = chrome.as_mut() { layout.set_flex_for_view(Some(chrome), 0); }
                if let Some(content) = content.as_mut() { layout.set_flex_for_view(Some(content), 1); }
            }
            window.layout();

            if self.initial_show_state != ShowState::HIDDEN {
                window.show();
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            *self.chrome_view.borrow_mut() = None;
            *self.content_view.borrow_mut() = None;
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            // Allow the window to close if the browser says it's OK.
            let content_view = self.content_view.borrow();
            let browser_view = content_view.as_ref().expect("BrowserView is None");
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
                RefCell::new(popup_browser_view.cloned()),
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
            let url = CefString::from(url);

            // Views is enabled by default (add `--use-native` to disable).
            let use_views = command_line.has_switch(Some(&CefString::from("use-native"))) == 0;

            // If using Views create the browser using the Views framework, otherwise
            // create the browser using the native platform framework.
            if use_views {
                // A fixed chrome bar, then Yahoo in a separate BrowserView
                // below it. CEF Views clips both views to their bounds.
                let chrome_html = include_str!("../../assets/chrome.html");

                let chrome_url = get_data_uri(chrome_html.as_bytes(), "text/html");
                let chrome_url = CefString::from(chrome_url.as_str());
                let mut chrome_client = self.default_client();
                let mut chrome_delegate = SimpleBrowserViewDelegate::new(runtime_style, 64);
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
                let mut delegate = SimpleWindowDelegate::new(
                    RefCell::new(chrome_view),
                    RefCell::new(browser_view),
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
