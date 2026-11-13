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

- macOS, x86_64 Linux, or Windows
- CMake + Ninja on `PATH`
- platform toolchain: Xcode Command Line Tools (macOS), GCC/GTK3/NSS (Linux),
  or Visual Studio C++ build tools (Windows)

CEF itself is downloaded automatically. The first build fetches the pinned
156.0.26 distribution into `./cef-binary` and caches it for later targets.
To use an existing distribution instead:

```sh
CEF_PATH=/path/to/cef_binary_156.0.26_linux64 cargo run
```

## Build and run

```sh
cargo run
```

`cargo run` targets `bundle`. On macOS it builds and launches the `.app`
(this is required for CEF's framework/ICU lookup); on Linux and Windows it
builds `target/bootstrap/debug/` and launches the executable directly. Logs
stream to the terminal. `./scripts/run.sh` does the same with Linux
`LD_LIBRARY_PATH` setup, `scripts/run.ps1` is its PowerShell equivalent, and
`./scripts/build.sh` builds without launching.

No window (for CI/screenshot work; macOS only):

```sh
cargo run -- --initial-show-state=hidden
```

Hidden smoke test (launches, checks DevTools/targets, kills the app):

```sh
./scripts/smoke-hidden.sh
```

Build without launching:

```sh
cargo run --bin bundle -- --build-only
```

Package an unsigned release `.app` zip (macOS):

```sh
./scripts/package-release.sh
```

On macOS, always launch through `cargo run`/the bundle: the raw binary cannot
load CEF's ICU outside the `.app`. On Linux/Windows the binary is self-contained
once CEF runtime files have been copied next to it.

