# Free Domain Browser — CEF prototype

Standalone CEF (Chromium Embedded Framework) browser prototype. This is a new
repository; the Tauri application is not modified.

Current state: macOS CEF window with a fixed HTML chrome bar in a top
`BrowserView` and the page in a second `BrowserView`. Yahoo loads top-level, so
there is no `X-Frame-Options` block and the page cannot paint over the chrome.

Implemented: dynamic tabs with titles/closing/shortcuts, address-bar and title
sync, back/forward/reload/home, security indicator, new-tab page, settings
persistence in Rust, registry conflict/claim pages with a per-session choice
memory, `fdn://` link handling, retry error pages, and a local chat UI. Settings
opens as an overlay below the chrome, chat as a 420 px side panel that follows
the active tab (with a compact drawer), and community reviews open in a reusable
dedicated review window.
System/light/dark theme, all seven skins and compact density are applied to the
chrome and internal pages; macOS gets a native app/Browser/Edit menu. The chat
page talks to the ported iroh service; set `FDN_CHAT_LOBBY_ID` (and optionally `FDN_CHAT_RELAY_URL`) to enable
the community relay, otherwise it reports “chat bootstrap is not configured”.
Still pending: a Developer ID-signed/notarized release, native MSI/NSIS/AppImage/
deb installers, and a production `fdn-chatd` lobby deployment. The chat
transport/protocol has been validated locally with two browser instances and a
headless gossip peer over the public relay.

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

Build and smoke-test the Linux build in a container, without a Linux
machine:

```sh
docker build -f Dockerfile.linux -t fdb-cef-linux .
docker run --rm --shm-size=1g fdb-cef-linux
```

The image builds the browser, the helper and the packaging binary, then runs
the browser on a virtual display and checks that it opens its page and its
chrome bar. The same two steps run in CI on every push.

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
distribution still needs a Developer ID signature. An optional DMG is created
when `hdiutil` is available:

```sh
./scripts/package-release.sh
```

Portable Linux and Windows archives can be built on their native platforms:

```sh
./scripts/package-linux.sh      # tar.gz, plus .deb when dpkg-deb is available
./scripts/package-windows.ps1  # zip, plus NSIS setup.exe when makensis exists
```

Tagged `v*` pushes run `.github/workflows/release.yml`, which builds those
portable archives, a Linux `.deb`, and a Windows NSIS setup when the tools are
available, and attaches them to the GitHub release. These are ad-hoc/unsigned
builds; MSI and AppImage packages are not produced.

Replace the ad-hoc signature with a Developer ID identity (macOS). The bundled CEF
helpers need hardened-runtime JIT/library entitlements, so the script signs
helpers first, then the framework, then the app, and rewrites the zip:

```sh
./scripts/sign-release.sh 'Developer ID Application: Example (TEAMID)'
```

Notarization still requires Apple credentials and is not part of the script:

```sh
xcrun notarytool submit target/FreeDomainBrowser-CEF.zip \
  --keychain-profile PROFILE --wait
xcrun stapler staple target/bundle/free-domain-browser-cef.app
```

On macOS, always launch through `cargo run`/the bundle: the raw binary cannot
load CEF's ICU outside the `.app`. On Linux/Windows the binary is self-contained
once CEF runtime files have been copied next to it.

