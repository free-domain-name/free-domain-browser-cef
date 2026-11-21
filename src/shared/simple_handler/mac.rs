use cef::*;
use objc2::{Message, rc::Retained};
use objc2_app_kit::{NSView, NSWindow};

fn window_from_browser(browser: Option<&mut Browser>) -> Option<Retained<NSWindow>> {
    let view_ptr = browser?.host()?.window_handle().cast::<NSView>();
    let view = unsafe { view_ptr.as_ref()? };
    let view = view.retain();
    view.window()
}

pub fn platform_show_window(browser: Option<&mut Browser>) {
    let Some(window) = window_from_browser(browser) else {
        return;
    };

    window.makeKeyAndOrderFront(Some(&window));
}
