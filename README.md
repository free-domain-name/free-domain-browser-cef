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
opens in a tab of its own, chat as a 420 px side panel that follows the active
tab (with a compact drawer), and community reviews open in a reusable dedicated
review window.
The Material light and dark theme and the compact density are applied to the
chrome and internal pages; the Browser and Edit menus exist on every platform:
a native menu bar on macOS and Windows, and a menu window on Linux, where CEF
draws its own X11 window and cannot host a native one. Press F10 or Alt+F to
open it. The chat page talks to the ported iroh service. The production relay
and the lobby key are compiled in, so the community chat works out of the box;
set `FDN_CHAT_LOBBY_ID` (and optionally `FDN_CHAT_RELAY_URL`), or write
`<app-config-dir>/chat-bootstrap.json`, to point it at another lobby.
Still pending: native MSI/AppImage installers; the Linux `.deb` and the Windows
NSIS setup are already produced by the packaging scripts. The macOS build is
signed with the Developer ID and notarized by Apple. The chat
transport/protocol has been validated locally with two browser instances and a
headless gossip peer over the public relay.

## Links

- Project site: <https://free-domain-name.github.io/>
- This build's page: <https://free-domain-name.github.io/free-domain-browser-cef/>
- The stable Tauri build's page: <https://free-domain-name.github.io/tauri-browser/>
- Changelog: [CHANGELOG.md](CHANGELOG.md)

## Preview build

CI publishes portable preview builds as a GitHub prerelease:
<https://github.com/free-domain-name/free-domain-browser-cef/releases>

Install the portable build in one command:

```sh
curl -fsSL https://free-domain-name.github.io/install.sh | sh
```

On Windows:

```powershell
irm https://free-domain-name.github.io/install.ps1 | iex
```

The script finds the newest release, checks its SHA-256 against
`checksums.txt` and unpacks it into your home directory. Nothing is written
outside it and nothing needs root. Set `FDB_VERSION=v0.1.2` to pin a release.

Pushing a `v*` tag builds the browser for macOS, Linux and Windows and
publishes the archives with a `checksums.txt`. The macOS build is signed with
the Developer ID and notarized by Apple, so it opens without the extra
confirmation; the Linux and Windows builds are unsigned and may still ask you
to confirm the first launch. The releases are
[v0.1.2](https://github.com/free-domain-name/free-domain-browser-cef/releases/tag/v0.1.2),
[v0.1.1](https://github.com/free-domain-name/free-domain-browser-cef/releases/tag/v0.1.1)
and
[v0.1.0](https://github.com/free-domain-name/free-domain-browser-cef/releases/tag/v0.1.0).

The stable build with installers is the Tauri one:
<https://github.com/free-domain-name/tauri-browser/releases>

## Verification

The journeys in `verification/journeys` are the written-down version of what
the browser must do, and the runner checks them against the real build:

```sh
scripts/verify.sh                # every journey
scripts/verify.sh first-launch   # one journey
```

It starts the actual application binary, drives the running window through
the DevTools protocol and looks at what is really there. No mock browser and
no fake network. The whole suite takes about half a minute. The same
journeys run in CI, where the browser starts hidden because the runner has
no screen.

The journeys run against every platform:

```sh
scripts/verify.sh                # macOS, on the build of this tree
scripts/verify-linux.sh          # Linux, inside the container of Dockerfile.linux
pwsh scripts/verify-windows.ps1  # Windows, against the packaged build
```

See [verification/README.md](verification/README.md) for the format of a
journey and for the list of what is covered.

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

Package a release `.app` zip (macOS). The bundle step signs the app, its
helpers, and the embedded framework so macOS will launch it locally. An
optional DMG is created when `hdiutil` is available:

```sh
./scripts/package-release.sh
```

To sign it with your own Developer ID and send it to the notary:

```sh
./scripts/sign-release.sh 'Developer ID Application: Example (TEAMID)'
xcrun notarytool submit target/FreeDomainBrowser-CEF.zip \
  --key <AuthKey.p8> --key-id <KEY_ID> --issuer <ISSUER_ID> --wait
xcrun stapler staple target/bundle/free-domain-browser-cef.app
```

The release workflow does all of this on its own when the repository holds the
`APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_API_KEY`,
`APPLE_API_KEY_ID` and `APPLE_API_ISSUER` secrets.

Portable Linux and Windows archives can be built on their native platforms:

```sh
./scripts/package-linux.sh      # tar.gz, plus .deb when dpkg-deb is available
./scripts/package-windows.ps1  # zip, plus NSIS setup.exe when makensis exists
```

Both packages carry the CEF runtime next to the browser, so they start without
any other file. The Linux package starts through a launcher that points the
loader at that runtime; a tarball cannot ship the setuid sandbox helper as
root, so the launcher says so and runs without the sandbox until the helper is
installed. The Debian package installs the helper as root and keeps the
sandbox.

Tagged `v*` pushes run `.github/workflows/release.yml`, which builds those
portable archives, a Linux `.deb`, and a Windows NSIS setup when the tools are
available, and attaches them to the GitHub release. The macOS build is signed
with the Developer ID and notarized when the Apple secrets are configured; MSI
and AppImage packages are not produced.

The Developer ID signature needs the hardened-runtime JIT/library entitlements,
so the script signs the nested libraries and the helpers first, then the
framework, then the app, and rewrites the zip. Notarization is a separate step
that needs Apple credentials; the workflow runs it with an App Store Connect API
key when the repository holds the secrets listed above.

On macOS, always launch through `cargo run`/the bundle: the raw binary cannot
load CEF's ICU outside the `.app`. On Linux/Windows the binary is self-contained
once CEF runtime files have been copied next to it.

