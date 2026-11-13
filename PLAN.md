# CEF migration plan

Status: prototype only. Goal is **not** finished.

## Constraints

- Work stays in `free-domain-browser-cef`; never commit to `tauri-browser`.
- Chrome/address bar is a fixed top BrowserView; the page must never cover it.
- No surprise GUI windows during development: use `--initial-show-state=hidden`,
  headless HTML screenshots, and container tests where possible.
- `cargo run` must launch the bundle, stream logs, and stop the browser and
  helpers on Ctrl-C.
- Every phase needs an automated test or a reproducible screenshot.

## Current state

- CEF 156 Alloy; fixed chrome (96 px) + one page BrowserView.
- Yahoo loads top-level; address bar cannot be covered.
- Tabs reuse one page view and switch its URL.
- `fdn-core` resolves registry/public names; navigation post is on the CEF UI thread.
- Settings and chat are internal `file://` pages; settings persist in `localStorage`.
- Chat has UI only.
- Container tests for `fdn-core` and headless chrome/settings/chat screenshots exist.
- Podman VM may be unavailable; scripts are the reproducible route.

## Phase 0 — stable dev loop (mostly done)

- [x] `cargo run` launches the `.app` bundle in the same terminal.
- [x] Ctrl-C kills browser + helper process group.
- [x] Hidden launch: `cargo run -- --initial-show-state=hidden`.
- [x] Document `CEF_PATH` / CMake setup.
- [ ] Add a hidden CEF smoke script that fails if the process exits early.
- [ ] Capture browser `stderr` to a log file while hidden for CI diagnostics.

## Phase 1 — CEF IPC bridge

Replace the synthetic `fdb.invalid` navigation hack with a typed bridge.

- [x] Browser command channel from chrome HTML (console-message transport; a
  `cefQuery` router can replace it later without changing command payloads).
- [x] Commands: `navigate`, `open_settings`, `open_chat`, `save_settings`,
  `back`, `forward`, `reload`, `home`.
- [ ] `new_tab`, `close_tab`, `select_tab` (blocked on Phase 3 tab model).
- [x] Keep `on_before_browse` only as a fallback for old links.
- [x] Ignore bridge traffic from page frames (only the chrome browser id is
  accepted).
- Tests: settings round-trip unit tests; hidden CDP smoke verified `navigate`,
  `settings`, `save_settings`, and `reload`.

## Phase 2 — chrome UI

- [x] Custom lightweight chrome: tab strip, address bar, back/forward/reload,
  security indicator, settings/chat buttons.
- [x] Tab titles and address bar sync from content page events.
- [x] Tab close/new/select and Cmd/Ctrl-T/W, Ctrl-Tab in the chrome page.
- [ ] Port the full Tauri toolbar/skins UI (deferred; current chrome is functional
  but not pixel-matched).
- [ ] Appearance/skins.
- Acceptance: hidden CDP verified title + URL updates and tab rendering.

## Phase 3 — independent tabs

- [ ] Rust tab model: id, url, title, history, view.
- [ ] Create one content BrowserView per tab; add/remove/hide on the CEF UI thread.
- [ ] Chrome IPC for new/close/select; active tab state.
- [ ] Preserve page state/history when switching; close only the selected view.
- [ ] New-tab page and tab titles from `on_title_change`.
- [ ] Keyboard shortcuts (Cmd-T/W, Ctrl-Tab).
- Tests: hidden CEF script creates 2 tabs, navigates different URLs, verifies both browsers exist and chrome stays fixed.

## Phase 4 — complete registry navigation

- [ ] Implement CEF `fdn://` scheme handler or local proxy so registry names keep
  their free-domain address instead of loading the record target directly.
- [ ] Wire `NavigationOutcome`: conflict picker, unregistered claim page, public fallback.
- [ ] Remember registry/public choices per session.
- [ ] Cache/negative TTL; error pages with retry.
- [ ] Address-bar display uses the registry name, not the target URL.
- Tests: `fdn-core` Linux tests plus mocked CEF scheme requests.

## Phase 5 — settings in Rust (partial)

- [x] JSON settings store (`home`, `cookies`, `dnt`) with sandbox-safe fallback.
- [x] `save_settings` bridge command; settings page loads values from the store
  through query parameters.
- [ ] Move the remaining appearance/proxy fields off `localStorage`.
- [ ] Apply settings to CEF request context/cache and chrome UI.
- Tests: round-trip save/load; empty home falls back to default.

## Phase 6 — chat backend

- [ ] Port `chat/{config,identity,protocol,policy,room,settings,transport}.rs`.
- [ ] Adapt app state/event bridge to CEF IPC (no Tauri events).
- [ ] Implement rooms, send/receive, identity, settings.
- [ ] Two-client local test; message delivery and reconnect.
- Acceptance: two hidden CEF instances exchange a message.

## Phase 7 — packaging and release

- [ ] Bundle `assets/`, locales, `cef.pak`, `icudtl.dat`.
- [ ] Code-sign app + helpers with a stable identity. Ad-hoc debug rebuilds
  invalidate Keychain "Always Allow"; debug currently passes
  `--use-mock-keychain` to avoid repeated "Chromium Safe Storage" prompts.
- [ ] Sandbox entitlements.
- [ ] Release profile with sandbox enabled (debug currently `no_sandbox`).
- [ ] `cargo run` remains a debug-only convenience.
- [ ] CI: Linux core tests, macOS bundle build, hidden smoke.

## Phase 8 — final QA

- [ ] `cargo test -p fdn-core` (Docker/Podman).
- [ ] `cargo test --bin free-domain-browser-cef`.
- [ ] Hidden CEF smoke; no leaked processes.
- [ ] YouTube/Yahoo + `frame-ancestors` site: page loads, chrome never covered.
- [ ] Tabs independent, settings persisted, chat works.
- [ ] Release `.app` opens from Finder.

## Suggested order

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8.

Phases 1 and 3 are the critical path. Do not port the full UI before the IPC
bridge and tab model exist, or the Tauri JS will be rewritten twice.

## Definition of done

A signed macOS `.app` that launches Yahoo and frame-blocking sites, keeps the
address bar fixed, supports independent tabs, resolves free-domain names through
the registry with conflict/claim flows, persists settings, and runs the chat
backend — with repeatable tests and screenshots produced without opening windows.
