#![cfg_attr(
    all(not(debug_assertions), not(feature = "sandbox"), target_os = "windows"),
    windows_subsystem = "windows"
)]

pub mod chat;
pub mod proxy;
pub mod settings;
pub mod shared;

#[cfg(target_os = "macos")]
mod mac;

#[cfg(not(all(feature = "sandbox", target_os = "windows")))]
fn main() -> Result<(), &'static str> {
    let _library = shared::load_cef();

    #[cfg(target_os = "macos")]
    {
        use cef::*;
        use std::ffi::CString;

        // Chromium's macOS password store asks for Keychain access on first
        // launch. That system prompt blocks the page's first paint, which made
        // the page BrowserView look empty until the user answered it. Keep the
        // app from touching the real keychain; saved password storage is not
        // implemented here anyway.
        let mut args: Vec<CString> = std::env::args()
            .map(|arg| CString::new(arg).map_err(|_| "Invalid command line argument"))
            .collect::<Result<_, _>>()?;
        for switch in ["--use-mock-keychain", "--password-store=basic"] {
            if !args.iter().any(|arg| arg.as_bytes() == switch.as_bytes()) {
                args.push(CString::new(switch).map_err(|_| "Invalid command line switch")?);
            }
        }
        let mut argv: Vec<*mut std::os::raw::c_char> = args
            .iter()
            .map(|arg| arg.as_ptr() as *mut std::os::raw::c_char)
            .collect();
        let main_args = cef::MainArgs {
            argc: argv.len() as i32,
            argv: argv.as_mut_ptr(),
        };

        let Some(cmd_line) = cef::command_line_create() else {
            return Err("Failed to create command line");
        };
        cmd_line.init_from_argv(main_args.argc, main_args.argv.cast());

        shared::run_main(&main_args, &cmd_line, std::ptr::null_mut());
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let args = cef::args::Args::new();
        let Some(cmd_line) = args.as_cmd_line() else {
            return Err("Failed to parse command line arguments");
        };

        shared::run_main(args.as_main_args(), &cmd_line, std::ptr::null_mut());
        Ok(())
    }
}

#[cfg(all(feature = "sandbox", target_os = "windows"))]
fn main() -> Result<(), &'static str> {
    Err("Running in sandbox mode on Windows requires bootstrap.exe or bootstrapc.exe.")
}
