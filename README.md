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

CEF on macOS must run from an `.app` bundle. The default `cargo run` target is
`bundle`: it builds the app bundle and then executes its binary in this terminal,
so browser logs stream to the console.

```sh
export CEF_PATH=/path/to/cef_binary_156.0.26_macosarm64
cargo run
```

No window (for CI/screenshot work):

```sh
cargo run -- --initial-show-state=hidden
```

`./scripts/run.sh` sets up local CEF/CMake paths and forwards the same flags.

Build the bundle without launching:

```sh
cargo run --bin bundle -- --build-only
```

The raw `free-domain-browser-cef` binary cannot load ICU outside the `.app`;
always launch through `cargo run`/the bundle.

The `cef-dll-sys` build error about a response body larger than 10 MB means
`CEF_PATH` is unset and the build script tried to download the CEF index.
