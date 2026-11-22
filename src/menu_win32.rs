// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

//! The Win32 native menu bar.
//!
//! CEF owns the window procedure of the browser window, so the menu commands
//! are picked up by a subclass that hands every other message back to CEF.
//!
//! This module only talks to Win32 and to the shared menu model, so it can be
//! compiled for Windows on its own.

#[cfg(target_os = "windows")]
mod win32 {
    // The Win32 names are spelled the way the platform spells them.
    #![allow(clippy::upper_case_acronyms)]

    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub type HWND = *mut c_void;
    pub type HMENU = *mut c_void;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LRESULT = isize;
    pub type WNDPROC = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

    const WM_COMMAND: u32 = 0x0111;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_APP_INSTALL_MENU: u32 = 0x8001;
    const MF_STRING: u32 = 0x0000;
    const MF_POPUP: u32 = 0x0010;
    const MF_SEPARATOR: u32 = 0x0800;
    const GWLP_WNDPROC: isize = -4;

    unsafe extern "system" {
        fn CreateMenu() -> HMENU;
        fn AppendMenuW(hmenu: HMENU, flags: u32, id: usize, text: *const u16) -> i32;
        fn SetMenu(hwnd: HWND, hmenu: HMENU) -> i32;
        fn DrawMenuBar(hwnd: HWND) -> i32;
        fn GetWindowLongPtrW(hwnd: HWND, index: isize) -> isize;
        fn SetWindowLongPtrW(hwnd: HWND, index: isize, value: isize) -> isize;
        fn PostMessageW(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> i32;
        fn CallWindowProcW(
            previous: WNDPROC,
            hwnd: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT;
    }

    /// The window procedure CEF installed, kept so every message that is not
    /// a menu command can be handed back to it.
    static PREVIOUS_PROC: AtomicUsize = AtomicUsize::new(0);

    /// The command id of the first menu entry. The ids follow the order of
    /// the shared menu model, so the n-th entry of the model owns the id
    /// `FIRST_COMMAND_ID + n`.
    pub const FIRST_COMMAND_ID: usize = 1001;

    /// The window procedure that answers the menu commands.
    pub unsafe extern "system" fn menu_wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_COMMAND {
            let id = wparam & 0xFFFF;
            if id >= FIRST_COMMAND_ID {
                crate::menu_win32::run_command(id);
                return 0;
            }
        }
        if message == WM_APP_INSTALL_MENU {
            // The menu is built here, in the message loop, and not in the
            // lifecycle callback that asked for it: changing the menu of the
            // window from inside the callback deadlocks the UI thread.
            unsafe {
                install_menu(hwnd);
            }
            return 0;
        }
        if message == WM_NCDESTROY {
            // Give the window back to CEF before it goes away.
            let previous = PREVIOUS_PROC.swap(0, Ordering::SeqCst);
            if previous != 0 {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, previous as isize);
                }
            }
        }
        let previous = PREVIOUS_PROC.load(Ordering::SeqCst);
        if previous == 0 {
            return 0;
        }
        let previous: WNDPROC = unsafe { std::mem::transmute(previous) };
        unsafe { CallWindowProcW(previous, hwnd, message, wparam, lparam) }
    }

    /// Builds the menu, installs it in the window and takes over the window
    /// procedure so the menu commands reach the browser.
    ///
    /// The menu itself is set later, from the message loop: doing it here
    /// would change the window from inside a CEF callback, which deadlocks
    /// the UI thread.
    pub fn install(hwnd: HWND) {
        unsafe {
            let previous = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
            if previous == 0 {
                return;
            }
            PREVIOUS_PROC.store(previous as usize, Ordering::SeqCst);
            SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                menu_wnd_proc as *const () as usize as isize,
            );
            if PostMessageW(hwnd, WM_APP_INSTALL_MENU, 0, 0) == 0 {
                // The message did not make it into the queue, so the menu is
                // left out rather than set from the wrong thread.
                eprintln!("[menu] the menu bar could not be posted to the window");
            }
        }
    }

    /// Builds the menu and hands it to the window.
    unsafe fn install_menu(hwnd: HWND) {
        let menu = unsafe { build_menu() };
        if menu.is_null() {
            return;
        }
        unsafe {
            if SetMenu(hwnd, menu) == 0 {
                return;
            }
            DrawMenuBar(hwnd);
        }
    }

    /// Builds the menu, installs it in the window and takes over the window
    /// procedure in one go.
    ///
    /// This is the variant that hangs the UI thread, so it is only used when
    /// it is asked for explicitly.
    pub fn install_direct(hwnd: HWND) {
        unsafe {
            let previous = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
            if previous == 0 {
                return;
            }
            PREVIOUS_PROC.store(previous as usize, Ordering::SeqCst);
            SetWindowLongPtrW(
                hwnd,
                GWLP_WNDPROC,
                menu_wnd_proc as *const () as usize as isize,
            );
            install_menu(hwnd);
        }
    }

    /// Builds the menu from the shared menu model.
    ///
    /// A popup is attached to its parent with `MF_POPUP` and the handle of the
    /// submenu, which is why the submenu is created first.
    unsafe fn build_menu() -> HMENU {
        let menu = unsafe { CreateMenu() };
        if menu.is_null() {
            return menu;
        }
        let mut id = FIRST_COMMAND_ID;
        for top in crate::menu::MENUS {
            let sub = unsafe { CreateMenu() };
            if sub.is_null() {
                continue;
            }
            for entry in top.entries {
                let mut text: Vec<u16> = entry.title.encode_utf16().collect();
                text.push(0);
                if entry.is_separator() {
                    unsafe {
                        AppendMenuW(sub, MF_SEPARATOR, 0, text.as_ptr());
                    }
                    continue;
                }
                unsafe {
                    AppendMenuW(sub, MF_STRING, id, text.as_ptr());
                }
                id += 1;
            }
            let mut title: Vec<u16> = top.title.encode_utf16().collect();
            title.push(0);
            unsafe {
                AppendMenuW(menu, MF_STRING | MF_POPUP, sub as usize, title.as_ptr());
            }
        }
        menu
    }
}

#[cfg(target_os = "windows")]
pub use win32::HWND;

/// The command id of the first menu entry.
#[cfg(target_os = "windows")]
pub const FIRST_COMMAND_ID: usize = win32::FIRST_COMMAND_ID;

/// The handler that runs a menu command.
#[cfg(target_os = "windows")]
pub type CommandHandler = fn(usize);

#[cfg(target_os = "windows")]
static COMMAND_HANDLER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Registers the handler that runs the menu commands.
///
/// The handler gets the command id of the chosen entry; the ids follow the
/// order of the shared menu model.
#[cfg(target_os = "windows")]
pub fn set_command_handler(handler: CommandHandler) {
    COMMAND_HANDLER.store(handler as usize, std::sync::atomic::Ordering::SeqCst);
}

/// Runs the menu command with the given id.
#[cfg(target_os = "windows")]
pub(crate) fn run_command(id: usize) {
    let handler = COMMAND_HANDLER.load(std::sync::atomic::Ordering::SeqCst);
    if handler == 0 {
        return;
    }
    let handler: CommandHandler = unsafe { std::mem::transmute(handler) };
    handler(id);
}

/// Installs the menu bar in the given window.
#[cfg(target_os = "windows")]
pub fn install(hwnd: HWND) {
    win32::install(hwnd);
}

/// Installs the menu bar in the given window without waiting for the message
/// loop.
///
/// The menu is then set from inside the CEF callback that asked for it, which
/// deadlocks the UI thread, so this is only used for testing.
#[cfg(target_os = "windows")]
pub fn install_direct(hwnd: HWND) {
    win32::install_direct(hwnd);
}
