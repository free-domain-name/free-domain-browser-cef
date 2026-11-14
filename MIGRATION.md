# Free Domain Browser — CEF migration

Standalone CEF prototype. This repo is separate from `tauri-browser`; no Tauri
commits are made here.

## Current architecture

- CEF 156, Alloy runtime style (multiple BrowserViews).
- One fixed chrome BrowserView (116 px: 32 px tabs, 64 px toolbar, 20 px status)
  above one page BrowserView.
- Chrome bar HTML runs from a `data:` URI; internal pages (`settings`, `chat`,
  `claim`, `conflict`, `unavailable`, `error`, `newtab`) are written to
  `file://` under `free-domain-browser-cef-ui` so they have a real origin.
- Registry navigation uses `fdn-core` off the UI thread; the result is posted
  back to the CEF UI thread.
- Settings persist through the shared settings store and are re-applied to the
  chrome immediately.
- Registry pages are served through the loopback `*.localhost` proxy so
  relative links, redirects, cookies and headers behave like Tauri's `fdn://`
  protocol.

## Verified

- CEF unit tests: 27 passed.
- `fdn-core` Linux tests: 14 passed, 5 live-network ignored.
- Headless Chromium checks of chrome, status bar, start page, settings and chat
  (Docker/Podman) with dynamic DOM assertions.
- macOS bundle build: `assets/mac/free-domain-browser-cef.icns` copied into
  `Contents/Resources` and `CFBundleIconFile` set.
- App icons ported from the Tauri project (`assets/icon.{svg,png}`, `icons/`).

## Build and test

```sh
export PATH=/tmp/buildtools/cmake/data/bin:/tmp/buildtools/bin:$PATH
export CARGO_HOME=/tmp/cef-cargo-home2
cargo build
cargo test --bin free-domain-browser-cef
cargo test -p fdn-core
```

Container checks:

```sh
./scripts/test-core-docker.sh
./scripts/test-docker.sh
```

Both scripts use the Docker CLI by default (`ENGINE=docker`).

## Tauri parity audit

The reference app is `/Users/ozkan/projects/tauri-browser` (read-only).

| Area | Tauri | CEF | Status |
| --- | --- | --- | --- |
| Registry lookup / resolver | sharded GitHub registry + public DNS | same `fdn-core` resolver | match |
| Registry-unavailable result | retry / continue-to-public modal | `unavailable.html` with retry and public continuation | match |
| Registry/public conflict | in-chrome modal | `conflict.html` chooser page | match (presentation differs) |
| Unregistered claim flow | steps, shard path, JSON, copy, GitHub, async recheck | same in `claim.html` | match |
| Community reviews | index counts + dedicated review window | index counts + review tab | partial (tab instead of window) |
| Tabs / history | iframe tabs with JS history | native BrowserView per tab with CEF history | match |
| Reopen tab / numeric tab keys | yes | yes | match |
| Shortcuts | native menu + chrome keys | chrome keys + content KeyboardHandler (`Cmd/Ctrl+[ ]`, `Alt+←/→`, `Ctrl+Tab`, `Cmd/Ctrl+1..9`) | match |
| Start page | themed inline page, resolver-routed samples | themed `newtab.html`, resolver-routed samples | match |
| Status/progress | statusbar pill + URL + mode, progress bar | 20 px statusbar (source + URL + mode) and progress bar | match |
| Theme | system/light/dark across chrome and pages | system/light/dark on chrome; internal pages remain dark | partial |
| Density | comfortable/compact across chrome | comfortable/compact setting; chrome compact mode | partial |
| Built-in skins | seven skins across chrome/pages | seven chrome palettes | partial |
| Custom CSS | 24k chrome CSS editor | 24k chrome CSS editor | match |
| Settings | in-chrome overlay, instant apply | settings tab, instant apply, back/close | partial (tab) |
| Application menu | native OS Browser/Edit menu | cross-platform overflow menu + keyboard handler | match (generalised) |
| External open | resolves registry target / public URL | resolves and opens registry target / public URL | match |
| Popup -> tab | new tab through resolver | `on_before_popup` creates a tab through the resolver | match |
| Downloads | no handler | automatic `~/Downloads` save with progress | CEF ahead |
| Registry proxy | redirects, header stripping, HTML rewrite, cookies | same, with COOP/COEP/CSP/XFO stripping | match |
| Chat backend | iroh gossip, signed profile cards, per-tab enable | same transport/protocol, per-tab enable, profile cooldown | match |
| Chat UI | side panel with profile view, ignore list, clear data | tab with profile view, ignore list, clear data, age gate, fingerprint/relay; no side-panel placement | partial (tab) |
| App icon / bundle metadata | full icon set + category/description/license | icon set and macOS `CFBundleIconFile`; bundle metadata is minimal | partial |
| Review window | dedicated window | review tab | partial |
| Mobile (Android/iOS) | supported | not supported by this CEF desktop prototype | not in scope |
| Installers / release CI | DMG/MSI/NSIS/AppImage/deb | macOS `.app` zip script only | missing |

## Remaining work

- Chat and settings are full tabs rather than Tauri's side panel/overlay. The
  features work, but the page is replaced while they are open.
- Theme and skins apply to the chrome; internal pages (settings/chat/claim) are
  still dark-only.
- Reviews open in a tab; Tauri uses a separate window.
- Desktop icons are wired for the macOS app bundle. Windows resource icons,
  Linux `.desktop` metadata and cross-platform installers are not built yet.
- Mobile targets are outside the CEF desktop prototype.
- Notarization still needs Apple credentials; ad-hoc signing is done by the
  bundle helper.
