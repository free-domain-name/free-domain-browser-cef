//! Windows uses CEF Views for its top-level window, so title/window handling is
//! already done by the Views delegate. These hooks exist for the Alloy fallback
//! and are intentionally no-ops.
use cef::{Browser, CefString};

pub fn platform_title_change(_browser: Option<&mut Browser>, _title: Option<&CefString>) {}

pub fn platform_show_window(_browser: Option<&mut Browser>) {}
