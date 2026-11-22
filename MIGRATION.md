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

- CEF unit tests: 33 passed.
- `fdn-core` Linux tests: 14 passed, 5 live-network ignored.
- Headless Chromium checks of chrome, status bar, start page, settings and chat
  (including compact side-panel mode) with dynamic DOM assertions.
- Hidden CEF CDP checks of the settings overlay, chat side panel hide/show while
  switching tabs, theme switch, review-window reuse and native menu installation.
- Linux `.deb` assembly was verified with `dpkg-deb`, and the Windows NSIS
  script was compiled to a PE installer with `makensis` (in Docker).
- The portable packages carry the CEF runtime: the Linux container smoke test
  unpacks the tarball and starts the packaged browser, and the Windows CI job
  runs the packaged executable and checks its DevTools endpoint.
- macOS bundle build: `assets/mac/free-domain-browser-cef.icns` copied into
  `Contents/Resources`, `CFBundleIconFile`, Utilities category, copyright,
  display name, high-resolution flag and the GPL LICENSE populated. The DMG was
  created, verified (CRC32 valid) and mounted with the app inside.
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
| Community reviews | index counts + dedicated review window | index counts + dedicated review window | match |
| Tabs / history | iframe tabs with JS history (native child-webview path is dormant — `content_mode()` hard-returns `"iframe"`, so CEF's native BrowserView port is the only live native implementation) | native BrowserView per tab with CEF history | match |
| Reopen tab / numeric tab keys | — (Tauri has neither) | Cmd+Shift+T reopen, Cmd/Ctrl+1..9 keys | CEF ahead |
| Shortcuts | native menu + chrome keys | chrome keys + content KeyboardHandler (`Cmd/Ctrl+[ ]`, `Alt+←/→`, `Ctrl+Tab`, `Cmd/Ctrl+1..9`) | match |
| Start page | themed inline page, resolver-routed samples | themed `newtab.html`, resolver-routed samples | match |
| Status/progress | statusbar pill + URL + mode, progress bar | transient bottom-left status bubble (hover target / engine status / toasts, auto-hides) and progress bar — persistent bar deliberately removed, modern-browser style | intentional divergence |
| Theme | system/light/dark across chrome and pages | system/light/dark across chrome and internal pages | match |
| Density | comfortable/compact across chrome | comfortable/compact chrome and compact internal-page spacing | match |
| Built-in skins | seven skins across chrome/pages | seven chrome and internal-page palettes | match |
| Custom CSS | 24k chrome CSS editor | 24k chrome CSS editor | match |
| Settings | in-chrome overlay, instant apply | fixed overlay below the chrome, instant apply, close/Escape | match |
| Application menu | native OS Browser/Edit menu | native app/Browser/Edit menu on macOS and Windows, a menu window on Linux (CEF draws its own X11 window there), plus the overflow fallback | match |
| External open | resolves registry target / public URL | resolves and opens registry target / public URL | match |
| Popup -> tab | new tab through resolver | `on_before_popup` creates a tab through the resolver | match |
| Downloads | no handler | automatic `~/Downloads` save with progress | CEF ahead |
| Registry proxy | redirects, header stripping, HTML rewrite, cookies | same, with COOP/COEP/CSP/XFO stripping | match |
| Chat backend | iroh gossip, signed profile cards, per-tab enable | same transport/protocol, per-tab enable, profile cooldown | match |
| Chat UI | side panel with profile view, ignore list, clear data | 420 px side panel with compact drawer, profile view, ignore list, clear data, age gate | match |
| App icon / bundle metadata | full icon set + category/description/license | full icon set; macOS category/copyright/display/high-res/icon metadata; Windows icon+version resources; Linux desktop entry; bundled LICENSE | match |
| Review window | dedicated window | dedicated top-level CEF window | match |
| Mobile (Android/iOS) | supported | not supported by this CEF desktop prototype | not in scope |
| Installers / release CI | DMG/MSI/NSIS/AppImage/deb | verified macOS `.app` zip + DMG, portable Linux tar.gz + `.deb`, portable Windows zip + NSIS setup; tagged CI workflow builds all three | partial (portable archives, Linux `.deb` and Windows NSIS setup; no MSI/AppImage) |

## Remaining work

- Installers are the portable archives, a Linux `.deb` and a Windows NSIS
  setup; MSI and AppImage packages are not produced.
- The Windows runtime is packaged but never launched in CI; only the Linux
  container build is verified today.
- Notarization still needs Apple credentials; ad-hoc signing is done by the
  bundle helper. Lobby deployment still needs a production `fdn-chatd`.
- Mobile targets are outside the CEF desktop prototype; CEF is a desktop
  embedding and this repository does not ship Android/iOS shells.
