# Free Domain Browser — CEF prototype

Standalone CEF (Chromium Embedded Framework) browser prototype. This is a new
repository; the Tauri application is not modified.

Current state: macOS CEF window with a fixed HTML chrome bar in a top
`BrowserView` and the page in a second `BrowserView`. Yahoo loads top-level, so
there is no `X-Frame-Options` block and the page cannot paint over the chrome.

The chrome bar now has tabs, navigation, settings and a chat shell. The full
Tauri UI, per-tab browser views and the chat backend are still pending.

## Requirements

- macOS arm64
- CEF binary distribution 156.0.26 (`CEF_PATH`)
- CMake + Ninja on `PATH`

## Build and run

CEF on macOS must run from an `.app` bundle. A raw `cargo run` binary cannot
find the framework's ICU resources, so use the wrapper:

```sh
export CEF_PATH=/path/to/cef_binary_156.0.26_macosarm64
./scripts/run.sh
```

`./scripts/run.sh --hidden` starts the same bundle without showing a window.

Manual equivalent:

```sh
export CEF_PATH=/path/to/cef_binary_156.0.26_macosarm64
cargo run --bin bundle
open target/bundle/free-domain-browser-cef.app
```

The `cef-dll-sys` build error about a response body larger than 10 MB means
`CEF_PATH` is unset and the build script tried to download the CEF index.
