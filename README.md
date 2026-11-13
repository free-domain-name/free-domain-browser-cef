# Free Domain Browser — CEF prototype

Standalone CEF (Chromium Embedded Framework) browser prototype. This is a new
repository; the Tauri application is not modified.

Current state: macOS CEF window with a fixed HTML chrome bar in a top
`BrowserView` and the page in a second `BrowserView`. Yahoo loads top-level, so
there is no `X-Frame-Options` block and the page cannot paint over the chrome.

The chrome bar is still a placeholder; navigation, tabs and the real UI from
`../tauri-browser/ui` are the next milestones.

## Requirements

- macOS arm64
- CEF binary distribution 156.0.26 (`CEF_PATH`)
- CMake + Ninja on `PATH`

## Build

```sh
export CEF_PATH=/path/to/cef_binary_156.0.26_macosarm64
export DYLD_FALLBACK_LIBRARY_PATH="$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
cargo run --bin bundle
open target/bundle/free-domain-browser-cef.app
```
