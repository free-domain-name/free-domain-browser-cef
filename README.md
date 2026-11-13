# Free Domain Browser — CEF prototype

Standalone CEF (Chromium Embedded Framework) browser prototype. This is a new
repository; the Tauri application is not modified.

Current state: macOS CEF window with a fixed HTML chrome bar in a top
`BrowserView` and the page in a second `BrowserView`. Yahoo loads top-level, so
there is no `X-Frame-Options` block and the page cannot paint over the chrome.

Implemented: tab strip with titles/closing/shortcuts, address-bar and title
sync, back/forward/reload/home, security indicator, new-tab page, settings
persistence in Rust, registry conflict/claim pages, and a local chat UI.
Tabs now use two independent content BrowserViews (separate history); dynamic
tab creation beyond two views is still pending. Still pending: iroh chat
backend, native registry scheme handler, and a signed release bundle.

## Requirements

- macOS arm64
- CEF binary distribution 156.0.26 (`CEF_PATH`)
- CMake + Ninja on `PATH`

## Build and run

CEF on macOS must run from an `.app` bundle. The default `cargo run` target is
`bundle`: it builds the app bundle and then executes its binary in this terminal,
so browser logs stream to the console.

```sh
# If CEF_PATH is not exported, the local `.cargo/config.toml` created by
# scripts/setup-cargo-env.sh is used.
cargo run
```

Hidden smoke test (launches, checks DevTools/targets, kills the app):

```sh
./scripts/smoke-hidden.sh
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

Package an unsigned release `.app` zip:

```sh
./scripts/package-release.sh
```

The raw `free-domain-browser-cef` binary cannot load ICU outside the `.app`;
always launch through `cargo run`/the bundle.

The `cef-dll-sys` build error about a response body larger than 10 MB means
`CEF_PATH` is unset and the build script tried to download the CEF index.
