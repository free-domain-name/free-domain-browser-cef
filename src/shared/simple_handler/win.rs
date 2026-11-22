//! Windows uses CEF Views for its top-level window, so title/window handling is
//! already done by the Views delegate. These hooks exist for the Alloy
//! fallback and are intentionally no-ops.
use cef::{Browser, ImplBrowser, ImplBrowserHost};

pub fn platform_show_window(_browser: Option<&mut Browser>) {}

/// Adds a Win32 menu bar to the window that holds the browser.
///
/// The menu itself lives in `crate::menu_win32`; this only hands it the
/// window handle of the browser and registers the command handler.
#[cfg(target_os = "windows")]
pub fn install_menu_bar(browser: Option<&mut Browser>) {
    use std::ffi::c_void;

    // The chrome browser can be reported more than once; one menu bar is
    // enough.
    static INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if INSTALLED.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    crate::menu_win32::set_command_handler(|id| {
        if let Some((script, page_script)) = crate::shared::simple_handler::menu_command(id) {
            crate::shared::simple_handler::run_menu_command(script, page_script);
        }
    });

    let Some(browser) = browser else { return };
    let Some(host) = browser.host() else { return };
    let handle = host.window_handle();
    if handle.is_null() {
        return;
    }
    crate::menu_win32::install(handle as *mut c_void);
    INSTALLED.store(true, std::sync::atomic::Ordering::SeqCst);
}
