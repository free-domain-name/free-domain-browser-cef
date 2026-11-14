# Free Domain Browser — CEF migration

Standalone CEF prototype. This repo is separate from `tauri-browser`; no Tauri
commits are made here.

## Current architecture

- CEF 156, Alloy runtime style (multiple BrowserViews).
- One fixed chrome BrowserView (height 96) above one page BrowserView.
- Chrome bar HTML is a `data:` URI; tabs, address bar, settings and chat buttons.
- Internal pages (`settings`, `chat`) are written to `file://` so they get a real
  origin and can use `localStorage`.
- Navigation IPC uses a synthetic `https://fdb.invalid/navigate?url=...` URL,
  intercepted in `on_before_browse`; page views are blocked from using it.
- `fdn-core` (workspace crate) resolves registry/public names off the UI thread;
  the result is posted back to the CEF UI thread and loaded in the page view.
- Settings persist through `localStorage`.

## Verified

- CEF prototype loads Yahoo and keeps the chrome view above the page.
- CEF unit tests: navigation URL parser.
- `fdn-core` Linux tests: 14 passed, 5 live-network ignored.
- Headless screenshots of chrome, settings and chat.

## Build and test

```sh
export PATH=/tmp/buildtools/cmake/data/bin:/tmp/buildtools/bin:$PATH
export CARGO_HOME=/tmp/cef-cargo-home2
export CEF_PATH=/tmp/cef-binary-156.0.26
cargo build
cargo test --bin free-domain-browser-cef
cargo test -p fdn-core
```

Container checks:

```sh
ENGINE=podman ./scripts/test-core-docker.sh
ENGINE=podman ./scripts/test-docker.sh
```

The Podman VM may be unavailable; the scripts are the reproducible path.

## Tauri parity audit

The reference app is `/Users/ozkan/projects/tauri-browser` (read-only). This
table records the current CEF prototype against its shipped behavior.

| Area | Tauri | CEF | Status |
| --- | --- | --- | --- |
| Registry lookup / resolver | sharded GitHub registry + public DNS | same `fdn-core` resolver | match |
| `fdn://` input and registry names | custom protocol proxy | resolves to target URL directly | partial |
| Registry/public conflict chooser | in-chrome modal | internal `conflict.html` page | match |
| Unregistered-name claim flow | shard path, JSON, copy, GitHub, recheck | same, in `claim.html` | match |
| Community reviews | index counts + dedicated review window | index counts + review tab (`stumble.html`) | partial |
| Tabs / per-tab history | native child webviews, JS history | one `BrowserView` per tab, CEF history | match |
| Global browser shortcuts | native menu forwards to chrome | chrome JS plus content `KeyboardHandler` | match |
| Appearance: theme | system/light/dark | system/light/dark in settings | match |
| Appearance: density | comfortable/compact | compact tab strip | partial |
| Appearance: built-in skins | seven skins | none; custom CSS only | missing |
| Appearance: custom CSS | 24k chrome CSS editor | 24k chrome CSS editor | match |
| Toolbar | home, external, cache, review, chat, settings | same plus Go; no progress bar | partial |
| Start page | samples and search | same samples and search | match |
| Chat backend | iroh gossip rooms | same transport/protocol | match |
| Chat panel UI | profile, users, ignore, settings, age gate | simplified chat page | partial |
| Native application menu | yes | no; keyboard handler covers shortcuts | missing |
| Registry proxy semantics | cookie/header/HTML rewriting | not implemented | missing |
| Popup -> tab behavior | `on_new_window` opens a tab | CEF default behavior not wired | missing |

## Remaining work

- Implement the `fdn://` resource-handler proxy (relative links, cookies,
  redirects, CSP/XFO stripping) instead of loading the record target directly.
- Port the built-in Tauri skins and the richer chat panel (profiles, users,
  ignore lists, per-tab enablement).
- Add a native application menu or verify every platform-level shortcut path.
- Wire popup windows to new tabs and add a download/progress surface.
- Separate CEF BrowserView per tab with preserved session/history is done;
  keep testing close/reopen and popup edge cases.
- Release `.app` bundling, signing, sandbox/helper entitlements and CEF resources
  are implemented; notarization still needs credentials.
