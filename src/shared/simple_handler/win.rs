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

    // How the menu bar is installed can be changed to tell the menu apart
    // from the rest of the Windows setup when the browser misbehaves:
    // "post" (the default) sets it from the message loop, "direct" sets it
    // from the lifecycle callback and "none" leaves it out.
    let mode = std::env::var("FDB_WINDOWS_MENU").unwrap_or_else(|_| "post".to_string());
    if mode == "none" {
        return;
    }

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
    // On Windows the window handle is the `HWND` wrapper around the pointer.
    if handle.0.is_null() {
        return;
    }
    let hwnd = handle.0 as *mut c_void;
    if mode == "direct" {
        crate::menu_win32::install_direct(hwnd);
    } else {
        crate::menu_win32::install(hwnd);
    }
    INSTALLED.store(true, std::sync::atomic::Ordering::SeqCst);
}
