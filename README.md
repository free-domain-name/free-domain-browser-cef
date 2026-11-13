# Free Domain Browser — CEF prototype

Standalone CEF (Chromium Embedded Framework) browser prototype. This is a new
repository; the Tauri application is not modified.

Current state: macOS CEF window with a fixed HTML chrome bar in a top
`BrowserView` and the page in a second `BrowserView`. Yahoo loads top-level, so
there is no `X-Frame-Options` block and the page cannot paint over the chrome.

Implemented: dynamic tabs with titles/closing/shortcuts, address-bar and title
sync, back/forward/reload/home, security indicator, new-tab page, settings
persistence in Rust, registry conflict/claim pages with a per-session choice
memory, `fdn://` link handling, retry error pages, and a local chat UI. The chat
page talks to the ported iroh service; set `FDN_CHAT_LOBBY_ID` (and optionally
`FDN_CHAT_RELAY_URL`) to enable the community relay, otherwise it reports
“chat bootstrap is not configured”.
Still pending: a Developer ID-signed/notarized release and a production
`fdn-chatd` lobby deployment. The chat transport/protocol has been validated
locally with two browser instances and a headless gossip peer over the public
relay; set `FDN_CHAT_LOBBY_ID` (and optionally `FDN_CHAT_RELAY_URL`) to point at
a deployed lobby.

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

For parallel instances, give each process its own `FDB_CACHE_DIR`,
`FDB_SETTINGS_PATH`, `FDN_CHAT_CONFIG_DIR` and `--remote-debugging-port`.

Build without launching:

```sh
cargo run --bin bundle -- --build-only
```

Package an ad-hoc-signed release `.app` zip (macOS). The bundle step signs the
app, its helpers, and the embedded framework so macOS will launch it locally;
distribution still needs a Developer ID signature:

```sh
./scripts/package-release.sh
```

Replace the ad-hoc signature with a Developer ID identity (macOS). The bundled CEF
helpers need hardened-runtime JIT/library entitlements, so the script signs
helpers first, then the framework, then the app:

```sh
./scripts/sign-release.sh 'Developer ID Application: Example (TEAMID)'
```

On macOS, always launch through `cargo run`/the bundle: the raw binary cannot
load CEF's ICU outside the `.app`. On Linux/Windows the binary is self-contained
once CEF runtime files have been copied next to it.

