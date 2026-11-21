//! Linux uses CEF Views for its top-level window, so title/window handling is
//! already done by the Views delegate. These hooks exist for the Alloy
//! fallback and are intentionally no-ops.
//!
//! CEF draws its own X11 window on Linux and does not host a GTK window, so
//! there is no native menu bar here: the menu is shown in a window of its own
//! instead (see `crate::menu::page_html`).
use cef::Browser;

pub fn platform_show_window(_browser: Option<&mut Browser>) {}
