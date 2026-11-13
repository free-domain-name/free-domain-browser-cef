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

## Remaining work

- Port the full Tauri chrome UI (styles, toolbar, status, appearance, skins).
- Separate CEF BrowserView per tab with preserved session/history.
- Registry/public conflict picker and unregistered-name claim page.
- Port chat backend (identity, rooms, transport, policy), not just the UI shell.
- Implement `fdn://` scheme/proxy for registry navigation instead of loading
  the record target directly.
- Release `.app` bundling, signing, sandbox/helper entitlements and CEF resources.
