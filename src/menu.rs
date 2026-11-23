// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

//! The native menu model shared by the Linux and Windows menu bars.
//!
//! macOS builds its menu with AppKit; the other two platforms use the GTK
//! and Win32 menu APIs. Keeping the entries here means a menu item means the
//! same thing on every platform, and that the commands stay in sync with the
//! chrome bar.

/// One entry of a native menu.
pub struct MenuEntry {
    /// The label shown in the menu.
    pub title: &'static str,
    /// JavaScript run in the chrome view. Empty when the entry does nothing
    /// in the chrome.
    pub script: &'static str,
    /// JavaScript run in the page of the active tab. Empty when the entry
    /// does not touch the page.
    pub page_script: &'static str,
}

impl MenuEntry {
    const fn new(title: &'static str, script: &'static str) -> Self {
        Self {
            title,
            script,
            page_script: "",
        }
    }

    const fn page(title: &'static str, page_script: &'static str) -> Self {
        Self {
            title,
            script: "",
            page_script,
        }
    }

    /// A separator line: neither a chrome nor a page command.
    const fn separator() -> Self {
        Self {
            title: "",
            script: "",
            page_script: "",
        }
    }

    pub fn is_separator(&self) -> bool {
        self.script.is_empty() && self.page_script.is_empty()
    }
}

/// A top level menu and its entries.
pub struct Menu {
    pub title: &'static str,
    pub entries: &'static [MenuEntry],
}

/// The Browser menu: navigation, tabs and the window itself.
pub const BROWSER_MENU: Menu = Menu {
    title: "Browser",
    entries: &[
        MenuEntry::new("Back", "window.fdbBack&&fdbBack()"),
        MenuEntry::new("Forward", "window.fdbForward&&fdbForward()"),
        MenuEntry::new("Reload", "window.fdbReload&&fdbReload()"),
        MenuEntry::separator(),
        MenuEntry::new("New Tab", "window.fdbNewTab&&fdbNewTab()"),
        MenuEntry::new("Close Tab", "window.fdbCloseActive&&fdbCloseActive()"),
        MenuEntry::new("Previous Tab", "window.fdbCycleTab&&fdbCycleTab(-1)"),
        MenuEntry::new("Next Tab", "window.fdbCycleTab&&fdbCycleTab(1)"),
        MenuEntry::separator(),
        MenuEntry::new("Open Location…", "window.fdbFocusUrl&&fdbFocusUrl()"),
        MenuEntry::separator(),
        MenuEntry::new("Settings", "window.fdbOpenSettings&&fdbOpenSettings()"),
        MenuEntry::separator(),
        MenuEntry::new("Quit", "window.fdbQuit&&fdbQuit()"),
    ],
};

/// The Edit menu: the text editing commands of the active page.
pub const EDIT_MENU: Menu = Menu {
    title: "Edit",
    entries: &[
        MenuEntry::page("Undo", "document.execCommand('undo')"),
        MenuEntry::page("Redo", "document.execCommand('redo')"),
        MenuEntry::separator(),
        MenuEntry::page("Cut", "document.execCommand('cut')"),
        MenuEntry::page("Copy", "document.execCommand('copy')"),
        MenuEntry::page("Paste", "document.execCommand('paste')"),
        MenuEntry::page("Select All", "document.execCommand('selectAll')"),
    ],
};

/// Every native menu, in the order they are shown.
pub const MENUS: &[Menu] = &[BROWSER_MENU, EDIT_MENU];

/// The menu of the chrome bar's "☰" button.
///
/// The chrome bar is a 79 pixel tall browser view: a dropdown drawn inside
/// the chrome page would be clipped away by the view. The button therefore
/// opens this menu as a native popup menu instead.
pub const TOOLBAR_MENU: Menu = Menu {
    title: "Free Domain Browser",
    entries: &[
        // Linux and Windows have no native menu bar, so the menu window is
        // the only way to reach the full menu there.
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        MenuEntry::new("Menu Bar", "window.fdbMenuBar&&fdbMenuBar()"),
        MenuEntry::new("New Tab", "window.fdbNewTab&&fdbNewTab()"),
        MenuEntry::new("Close Tab", "window.fdbCloseActive&&fdbCloseActive()"),
        MenuEntry::separator(),
        MenuEntry::new("Reload", "window.fdbReload&&fdbReload()"),
        MenuEntry::new("Back", "window.fdbBack&&fdbBack()"),
        MenuEntry::new("Forward", "window.fdbForward&&fdbForward()"),
        MenuEntry::separator(),
        MenuEntry::new("Settings", "window.fdbOpenSettings&&fdbOpenSettings()"),
        MenuEntry::new("Community Chat", "window.fdbToggleChat&&fdbToggleChat()"),
        MenuEntry::new("Open in System Browser", "window.fdbOpenExternal&&fdbOpenExternal()"),
        MenuEntry::new("Clear Registry Cache", "window.fdbClearCache&&fdbClearCache()"),
        MenuEntry::separator(),
        MenuEntry::new("About Free Domain Browser", "window.fdbAbout&&fdbAbout()"),
        MenuEntry::new("Quit", "window.fdbQuit&&fdbQuit()"),
    ],
};

/// The first command id of the toolbar menu.
///
/// CEF reserves the ids below `MENU_ID_USER_FIRST` (26500) for itself.
pub const TOOLBAR_MENU_FIRST_COMMAND_ID: i32 = 26_500;

/// The entries of the toolbar menu, in the order they are shown.
///
/// A separator has no command id.
pub fn toolbar_entries() -> impl Iterator<Item = (Option<i32>, &'static MenuEntry)> {
    let mut id = TOOLBAR_MENU_FIRST_COMMAND_ID;
    TOOLBAR_MENU.entries.iter().map(move |entry| {
        if entry.is_separator() {
            (None, entry)
        } else {
            let command_id = id;
            id += 1;
            (Some(command_id), entry)
        }
    })
}

/// The toolbar entry with the given command id.
pub fn toolbar_entry(command_id: i32) -> Option<&'static MenuEntry> {
    let index = usize::try_from(command_id.checked_sub(TOOLBAR_MENU_FIRST_COMMAND_ID)?).ok()?;
    TOOLBAR_MENU
        .entries
        .iter()
        .filter(|entry| !entry.is_separator())
        .nth(index)
}

/// Every entry of every menu, in the order they are shown.
///
/// The index of an entry in this list is the id the menu page sends back
/// when an item is chosen.
pub fn entries() -> impl Iterator<Item = &'static MenuEntry> {
    MENUS
        .iter()
        .flat_map(|menu| menu.entries.iter())
        .filter(|entry| !entry.is_separator())
}

/// The entry with the given id.
pub fn entry(id: usize) -> Option<&'static MenuEntry> {
    entries().nth(id)
}

/// The menu as a small HTML page.
///
/// Linux cannot use a native menu bar: CEF draws its own X11 window there and
/// does not host a GTK window, so the same menu is shown in a CEF window
/// instead.
pub fn page_html() -> String {
    let mut body = String::new();
    let mut id = 0usize;
    for menu in MENUS {
        body.push_str(&format!("<section><h2>{}</h2>", html_escape(menu.title)));
        for entry in menu.entries {
            if entry.is_separator() {
                body.push_str("<hr>");
                continue;
            }
            body.push_str(&format!(
                "<button type=\"button\" data-id=\"{id}\">{}</button>",
                html_escape(entry.title)
            ));
            id += 1;
        }
        body.push_str("</section>");
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Menu</title>
<style>
{kit}
body{{margin:0;padding:8px;background:var(--chrome);color:var(--text);font:13px -apple-system,system-ui,sans-serif;min-width:220px}}
h2{{margin:6px 4px 4px;font-size:11px;text-transform:uppercase;letter-spacing:.06em;opacity:.6}}
button{{display:block;width:100%;text-align:left;border:0;background:transparent;color:var(--text);font:inherit;padding:7px 10px;border-radius:7px;cursor:pointer}}
button:hover{{background:var(--button-hover)}}
hr{{border:0;border-top:1px solid var(--border);margin:5px 4px}}
</style>
</head>
<body>
{body}
<script>
document.addEventListener('click',function(e){{
  var button=e.target.closest&&e.target.closest('button[data-id]');
  if(!button)return;
  console.log('fdb:'+JSON.stringify({{cmd:'menu_action',id:Number(button.dataset.id)}}));
}});
</script>
</body>
</html>
"#,
        kit = crate::shared::simple_handler::UI_KIT_CSS,
        body = body
    )
}

/// Escapes the characters that matter in HTML text.
fn html_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_menu_page_lists_every_entry_in_order() {
        let html = super::page_html();
        for (id, entry) in super::entries().enumerate() {
            let expected = format!("data-id=\"{id}\">{}</button>", entry.title);
            assert!(html.contains(&expected), "the page is missing {expected}");
        }
        assert_eq!(
            super::entries().count(),
            html.matches("data-id=").count(),
            "the page has more or fewer entries than the model"
        );
    }

    #[test]
    fn every_entry_runs_a_command() {
        for entry in super::entries() {
            assert!(
                !entry.script.is_empty() || !entry.page_script.is_empty(),
                "{} does nothing",
                entry.title
            );
        }
    }

    use super::*;

    #[test]
    fn every_menu_has_entries() {
        for menu in MENUS {
            assert!(!menu.title.is_empty());
            assert!(!menu.entries.is_empty());
        }
    }

    #[test]
    fn an_entry_runs_something() {
        for menu in MENUS {
            for entry in menu.entries {
                assert!(
                    !entry.is_separator() || entry.title.is_empty(),
                    "a separator has no label"
                );
                assert!(
                    entry.is_separator()
                        || !entry.script.is_empty()
                        || !entry.page_script.is_empty(),
                    "{} has no command",
                    entry.title
                );
            }
        }
    }
}
