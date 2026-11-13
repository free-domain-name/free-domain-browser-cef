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
tab creation beyond two views is still pending. The chat page talks to the ported iroh service; set `FDN_CHAT_LOBBY_ID` (and
optionally `FDN_CHAT_RELAY_URL`) to enable the community relay, otherwise it
reports “chat bootstrap is not configured”.
Still pending: native registry scheme handler and a signed release bundle.

## Requirements

- macOS arm64 or x86_64 Linux
- CEF binary distribution 156.0.26 (`CEF_PATH`)
- CMake + Ninja on `PATH` (and the usual CEF Linux runtime libraries)

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

### Linux

The CEF build script's automatic download currently fails with
`response body is larger than request limit: 10485760`; download the
distribution explicitly instead:

```sh
./scripts/fetch-cef-linux.sh          # writes ./cef-binary
./scripts/run.sh                     # builds target/bootstrap and launches
```

If you already unpacked CEF yourself, point at it:

```sh
CEF_PATH=/opt/cef_binary_156.0.26_linux64 ./scripts/run.sh
```

On Linux `cargo run` uses the `bundle` launcher too, but there is no `.app`;
it builds `target/bootstrap/debug/free-domain-browser-cef`, sets
`LD_LIBRARY_PATH` to `CEF_PATH`, and launches it. Debug builds disable the
Chromium sandbox so a non-setuid `chrome-sandbox` does not block startup.

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
