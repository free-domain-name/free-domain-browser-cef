# CEF migration plan

Status: desktop functionality is ported, including settings/chat overlays, a dedicated review window and themed internal pages. Remaining work is packaging polish, density inside content pages, and platform-limited mobile support. See `MIGRATION.md`.

## Constraints

- Work stays in `free-domain-browser-cef`; never commit to `tauri-browser`.
- Chrome/address bar is a fixed top BrowserView; the page must never cover it.
- No surprise GUI windows during development: use `--initial-show-state=hidden`,
  headless HTML screenshots, and container tests where possible.
- `cargo run` must launch the bundle, stream logs, and stop the browser and
  helpers on Ctrl-C.
- Every phase needs an automated test or a reproducible screenshot.

## Current state

- CEF 156 Alloy; fixed chrome (116 px) + one page BrowserView per tab, created on demand.
- Yahoo loads top-level; address bar cannot be covered.
- Each tab owns a stable BrowserView, so switching preserves page state/history;
  tabs are created and destroyed dynamically with no fixed cap.
- `fdn-core` resolves registry/public names; navigation post is on the CEF UI thread.
- Settings and chat are internal `file://` pages; settings persist in `localStorage`. Settings opens as a fixed overlay below the chrome and chat as a 420 px side panel.
- Chat has a working iroh backend, profile cards, ignore/clear controls, per-tab enablement and a compact side-panel drawer.
- Container tests for `fdn-core` and headless chrome/settings/chat screenshots exist, including compact chat panel mode.
- Reviews open in a dedicated CEF top-level window; system/light/dark theme and all seven skins reach the chrome and internal pages.
- Container scripts default to the Docker CLI; see README/MIGRATION for the current VM state.

## Phase 0 — stable dev loop (mostly done)

- [x] `cargo run` launches the `.app` bundle in the same terminal.
- [x] Ctrl-C kills browser + helper process group.
- [x] Hidden launch: `cargo run -- --initial-show-state=hidden`.
- [x] Document `CEF_PATH` / CMake setup.
- [x] Add a hidden CEF smoke script that fails if the process exits early.
- [x] Capture browser `stderr` to a log file while hidden for CI diagnostics.

## Phase 1 — CEF IPC bridge

Replace the synthetic `fdb.invalid` navigation hack with a typed bridge.

- [x] Browser command channel from chrome HTML (console-message transport; a
  `cefQuery` router can replace it later without changing command payloads).
- [x] Commands: `navigate`, `open_settings`, `open_chat`, `save_settings`,
  `back`, `forward`, `reload`, `home`.
- [x] `new_tab`, `close_tab`, `select_tab` with uid-based routing.
- [x] Keep `on_before_browse` only as a fallback for old links.
- [x] Ignore bridge traffic from page frames (chrome browser id or chrome page
  URL is required).
- Tests: settings round-trip unit tests; hidden CDP smoke verified `navigate`,
  `settings`, `save_settings`, `reload`, and the `newTab`/`closeTab` UI path.

## Phase 2 — chrome UI

- [x] Custom lightweight chrome: tab strip, address bar, back/forward/reload,
  security indicator, settings/chat buttons.
- [x] Tab titles and address bar sync from content page events.
- [x] Tab close/new/select and Cmd/Ctrl-T/W, Ctrl-Tab, Cmd-L/R.
- [x] Real new-tab page with search and shortcuts.
- [x] Ported Tauri toolbar controls, skins, density and custom CSS.
- [x] Appearance/skins.
- Acceptance: hidden CDP verified title + URL updates and tab rendering.

## Phase 3 — independent dynamic tabs

- [x] Rust tab model: uid + browser id, url/title in chrome.
- [x] One content BrowserView per open tab, created on demand.
- [x] Chrome IPC for new/close/select; active uid state.
- [x] Preserve page state/history when switching; close removes the view and
  releases the browser.
- [x] New-tab page and tab titles from `on_title_change`.
- [x] Keyboard shortcuts (Cmd/Ctrl-T/W, Ctrl-Tab, Cmd/Ctrl-1..9).
- [x] Replace the precreated eight-view pool with dynamic create/destroy so tab
  count is unbounded.
- Tests: hidden CEF smoke verifies DevTools targets; CDP exercises the real
  `newTab`/`closeTab` UI path and confirms the target appears/disappears and
  chrome stays fixed.

## Phase 4 — registry navigation

- [x] Conflict picker and unregistered claim pages.
- [x] Conflict choice routes registry/public target.
- [x] Address bar keeps the typed registry name on internal pages.
- [x] `fdn://` link interception / persistent free-domain address in the address bar.
- [x] Remember registry/public choices per session (in-process map).
- [x] Error pages with retry; registry-unavailable page with retry/continue-public.

## Phase 5 — settings in Rust

- [x] JSON settings store (`home`, `cookies`, `dnt`) with sandbox-safe fallback.
- [x] `save_settings` bridge command; settings page loads values from the store
  through query parameters.
- [x] Move the remaining appearance fields off `localStorage` (compact tabs,
  status visibility).
- [x] Apply appearance settings to the chrome UI; cookies/DNT are applied per
  request.
- [x] Settings drive the loopback proxy cookie/DNT/cache behavior.
- Tests: round-trip save/load; empty home falls back to default.

## Phase 6 — chat (ported client)

- [x] Functional local chat UI: rooms, send, per-room localStorage history.
- [x] Port `chat/{config,identity,protocol,policy,room,settings,transport}.rs`
  and the iroh transport into `src/chat` with a UI-free event sink.
- [x] Chat page uses `fdbchat:` commands (`snapshot`, `confirm_age`, `enable`,
  `send`, `nickname`, ignore) and polls snapshots.
- [x] Two-client test over the public relay with a headless gossip peer:
  both clients saw presence and exchanged messages in each direction.
  A production `fdn-chatd` lobby bootstrap is still an external deployment.

## Phase 7 — packaging and release (partial)

- [x] `scripts/package-release.sh` builds an ad-hoc-signed release `.app` zip;
  verified launch + 147 MB zip.
- [x] Release build path for app + helper (`--release`).
- [x] Bundle locales/`cef.pak`/`icudtl.dat` (provided by the CEF framework; verified in the app bundle).
- [x] Code-sign app + helpers with a stable identity via `scripts/sign-release.sh`;
  verified with `Developer ID Application: Ozkan Pakdil (TA5X5C9T57)`, and the
  signed zip was rewritten. Notarization remains pending Apple credentials.
  Debug/release bundles are ad-hoc signed after assembly so macOS does not kill
  them with an invalid signature.
- [x] Hardened-runtime sandbox entitlements (`assets/entitlements.mac.plist`).
- [x] CI smoke workflow (`.github/workflows/ci.yml`).

## Phase 8 — final QA

- [x] `cargo test -p fdn-core` (host run; Docker/Podman script remains available).
- [x] `cargo test --bin free-domain-browser-cef`.
- [x] Hidden CEF smoke; no leaked processes.
- [x] YouTube/Yahoo navigation, dynamic tabs, and settings persistence verified
  through the hidden CEF harness.
- [x] Ad-hoc-signed release bundle launches hidden and serves DevTools.
- [x] Chat two-client local validation (presence + bidirectional messages).
- [ ] Production `fdn-chatd` lobby deployment and notarization credentials.

## Suggested order

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8.

Phases 1 and 3 are the critical path. Do not port the full UI before the IPC
bridge and tab model exist, or the Tauri JS will be rewritten twice.

## Definition of done

A signed macOS `.app` that launches Yahoo and frame-blocking sites, keeps the
address bar fixed, supports independent tabs, resolves free-domain names through
the registry with conflict/claim flows, persists settings, and runs the chat
backend — with repeatable tests and screenshots produced without opening windows.


## Developer setup

CEF is provisioned by Cargo on all desktop platforms: `.cargo/config.toml`
defaults `CEF_PATH` to `./cef-binary`, and the vendored `download-cef` patch
raises the CDN index limit so a clean `cargo run` can download the pinned
156.0.26 archive. Override `CEF_PATH` to use an existing distribution.
