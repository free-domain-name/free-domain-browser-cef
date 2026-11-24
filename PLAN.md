# CEF migration plan

Status: the desktop port is done and the journeys hold it down. The audit
behind this plan (three code reads plus a measured run of the browser in the
Linux container) found a short list of real defects — a toolbar that is 17 px
too tall, a custom CSS that never reaches the chrome, a home setting that the
Home button ignores — and a set of half-features that the interface promises
but does not keep. Phase 9 is that list, in the order it should be done.
Everything below Phase 9 is history and stays as it was.

## What the verification proves today

| gate | result |
| --- | --- |
| Linux container, 14 journeys | **125 of 125 steps pass** (`scripts/verify-linux.sh`) |
| macOS, 13 journeys | 104 of 105 — the chat step failed on a fault in the runner itself, since fixed but not re-run on macOS |
| `cargo test -p fdn-core` | 14 pass, 5 live-network ignored |
| `cargo test --bin free-domain-browser-cef` | 34 pass |
| `cargo fmt --all --check`, `cargo clippy -D warnings` | clean |

The journeys run on Linux and Windows when the CI workflow is started by hand,
and the cheap gates run once a week. macOS runs on the `macos-latest` runner.

## Constraints

- Work stays in `free-domain-browser-cef`; never commit to `tauri-browser`.
- Chrome/address bar is a fixed top BrowserView; the page must never cover it.
- No surprise GUI windows during development: use `--initial-show-state=hidden`,
  headless HTML screenshots, and container tests where possible.
- `cargo run` must launch the bundle, stream logs, and stop the browser and
  helpers on Ctrl-C.
- Every phase needs an automated test or a reproducible screenshot.

## Current state

- CEF 156 Alloy; fixed chrome (79 px) + one page BrowserView per tab, created
  on demand. The chrome needs 96 px, so its bottom is cut off — see 9.1.
- Each tab owns a stable BrowserView, so switching preserves page state/history;
  tabs are created and destroyed dynamically with no fixed cap.
- `fdn-core` resolves registry/public names; navigation post is on the CEF UI thread.
- A name typed into the chrome is handed to the resolver as it was typed: the
  registry is asked first, the public web comes second, and a name that is in
  both opens the chooser. The choice is remembered for the session.
- Settings and chat are internal `file://` pages; settings persist in the Rust
  settings store and hydrate through URL parameters. Settings opens as a tab of
  its own and chat as a 420 px side panel.
- Chat has a working iroh backend, profile cards, ignore/clear controls,
  per-tab enablement and a compact side-panel drawer. It talks to one hard
  coded lobby; see 9.2.7.
- The Material light and dark theme reach the chrome and the internal pages.
  The compact density reaches the chrome as its own rules and the internal
  pages through the shared kit — the two are not the same, see 9.2.1.
- The community reviews open in the system browser, because the review page
  asks for a GitHub login and needs a real browser. The old dedicated review
  window was removed on purpose; the documentation has been corrected.
- The Browser and Edit menus exist on every platform: a native menu bar on
  macOS and Windows, and a menu window on Linux, where CEF draws its own X11
  window. F10 only reaches the menu on Linux and Windows — see 9.1.4.
- Container scripts default to the Docker CLI; see README/MIGRATION for the
  current VM state.

## Phase 0 — stable dev loop (done)

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
- [x] Ported Tauri toolbar controls, density and custom CSS.
- [x] Appearance: one Material theme (light and dark) and the compact density.
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
- [x] All fields live on the store: the appearance fields hydrate from query
  parameters and save back through the bridge.
- [x] Apply appearance settings to the chrome UI; cookies/DNT are applied per
  request.
- [x] Settings drive the loopback proxy cookie/DNT behavior (the proxy always
  forwards same-origin `Cookie`; third-party blocking is enforced once in the
  CEF request handler. There is no proxy HTTP cache; only the registry request
  cache is cleared).
- Tests: round-trip save/load; empty home falls back to default.

## Phase 6 — chat (ported client)

- [x] Functional local chat UI: send, session-only in-memory message history
  (200-message ring, matching Tauri; no per-room localStorage history).
- [x] Port `chat/{config,identity,protocol,policy,room,settings,transport}.rs`
  and the iroh transport into `src/chat` with a UI-free event sink.
- [x] Chat page uses `fdbchat:` commands (`snapshot`, `confirm_age`, `enable`,
  `send`, `nickname`, ignore) and polls snapshots.
- [x] Two-client test over the public relay with a headless gossip peer:
  both clients saw presence and exchanged messages in each direction.
- [ ] A production `fdn-chatd` lobby deployment — see 9.2.7.

## Phase 7 — packaging and release (partial)

- [x] `scripts/package-release.sh` builds an ad-hoc-signed release `.app` zip;
  verified launch + 147 MB zip.
- [x] Release build path for app + helper (`--release`).
- [x] Bundle locales/`cef.pak`/`icudtl.dat` (provided by the CEF framework; verified in the app bundle).
- [x] Code-sign app + helpers with a stable identity via `scripts/sign-release.sh`;
  verified with `Developer ID Application: Ozkan Pakdil (TA5X5C9T57)`, and the
  signed zip was rewritten. The nested libraries of the Chromium framework are
  signed as well, because the notary rejects an archive whose nested code
  carries only a linker signature.
  Debug/release bundles are ad-hoc signed after assembly so macOS does not kill
  them with an invalid signature.
- [x] Notarize the signed build with Apple and staple the ticket; verified with
  `spctl --assess`: "accepted, source=Notarized Developer ID". The release
  workflow repeats the whole chain from the repository secrets.
- [x] Hardened-runtime sandbox entitlements (`assets/entitlements.mac.plist`).
- [x] CI smoke workflow (`.github/workflows/ci.yml`).

## Phase 8 — final QA

- [x] `cargo test -p fdn-core` (host run; Docker/Podman script remains available).
- [x] `cargo test --bin free-domain-browser-cef`.
- [x] Hidden CEF smoke; no leaked processes.
- [x] Navigation, dynamic tabs, and settings persistence verified through the
  hidden CEF harness and the journeys.
- [x] Ad-hoc-signed release bundle launches hidden and serves DevTools.
- [x] Chat two-client local validation (presence + bidirectional messages).
- [ ] The defects and gaps of Phase 9.

## Phase 9 — closing the gaps the audit found

Every item below was found by reading the code and by measuring the running
browser in the Linux container. The measurements are in brackets.

### 9.1 Defects a user can see (do these first)

**9.1.1 The toolbar is cut off.** The chrome view is 79 px tall
(`src/shared/simple_app.rs:8`, `src/shared/simple_handler/mod.rs:153`), but the
chrome draws a 32 px tab strip and a 64 px toolbar
(`assets/chrome.html:4`, `assets/chrome.html:12`) — 96 px in total.
[Measured: the view is 79 px, the toolbar's bottom edge is at 96 px, so 17 px
of the toolbar, including the bottom of the Go button, is clipped.] Fix: make
`CHROME_HEIGHT` 96 and keep the two heights in one place, so the chrome and the
view cannot drift apart again. Acceptance: a journey that reads
`document.querySelector('.toolbar').getBoundingClientRect().bottom` and
compares it with `window.innerHeight`.

**9.1.2 The custom CSS does not reach the chrome.** The settings promise
`:root{--accent:#ff6600}` in the placeholder (`assets/settings.html:33`), but
the shared kit is injected at `</head>` (`src/shared/simple_app.rs:471-475`),
which is *after* the custom CSS `<style>` (`assets/chrome.html:29`), so at the
same specificity the kit wins. [Measured: after saving
`:root{--accent:#ff6600}`, the computed `--accent` of the chrome is still
`#2563eb`.] The internal pages inject the custom CSS after the kit, so it works
there — the two behave differently. Fix: inject the custom CSS after the kit on
the chrome as well, and say in the settings that it reaches the internal pages
too (it already does). Acceptance: a journey that saves a custom CSS and reads
the computed `--accent` of the chrome.

**9.1.3 The Home button ignores the home setting.** The setting is read only at
startup (`src/shared/simple_app.rs:439-448`); the Home button always loads the
start page (`src/shared/simple_handler/mod.rs:1825-1836`). [Measured: with
`home` set to `http://example.com`, the Home button still opened the start
page.] The settings page calls it "Home page" and does not say it only affects
the start. Fix: either make the Home button go to the configured home, or
rename the setting to "Start page" and say so. The first is what a user
expects. Acceptance: a journey that sets the home page, presses Home and
checks the address.

**9.1.4 F10 is a dead key in a content page on macOS.** The native key handler
swallows F10 (`src/shared/simple_handler/mod.rs:1996-1999`) and calls
`open_menu_window()`, whose body is empty outside Linux and Windows
(`src/shared/simple_handler/mod.rs:84-93`). The chrome's own F10 works
everywhere (it goes through the page to a native popup), so the key works in
the chrome and does nothing in the page. Fix: on macOS let the key through
instead of consuming it, or open the native menu bar from it.

**9.1.5 The security indicator lies.** The icon is only updated for http(s)
URLs (`src/shared/simple_handler/mod.rs:564`), so it keeps the last page's
state on the internal pages, and its label always says "Secure connection"
(`assets/chrome.html:37`) even when the icon warns about http. Fix: show a
neutral state for the internal pages and correct the label.

**9.1.6 The reload button turns into a stop button but still reloads.** The
glyph changes while the page loads (`assets/chrome.html:263`) but the command
is always `reload` (`assets/chrome.html:280`), and no stop command exists. Fix:
add a `stop` bridge command, or drop the stop glyph.

### 9.2 Features that are half done

**9.2.1 The density is implemented twice.** The chrome uses its own
`body.compact` rules (`assets/chrome.html:26`) and never sets `data-density`,
so the shared kit's density block (`assets/ui-kit.css:166-180`) never reaches
it. [Measured: with the compact density, the tab strip is 26 px instead of
32 px, but the font size stays 13 px in both densities.] Fix: let the chrome
use the shared kit's density, so the text shrinks as well.

**9.2.2 The custom CSS is missing from newly created internal pages.** The
page bootstrap carries the theme and the density but not the custom CSS
(`src/shared/simple_handler/mod.rs:192`); it only reaches an open page through
the live script after a save (`src/shared/simple_handler/mod.rs:211-221`). A
new tab or a reloaded settings tab is therefore without it. Fix: put the
custom CSS into the bootstrap as well.

**9.2.3 The settings live in the macOS folder on every platform.**
`src/settings.rs:72-75` joins `Library/Application Support` under `$HOME`, so
Linux and Windows users get a stray `~` directory. Fix: use the platform's
own config directory.

**9.2.4 The settings are read from disk on every request.**
`src/shared/simple_handler/mod.rs:2127,2186` and `src/proxy.rs:365` read and
parse the file per resource request. Fix: keep the settings in memory and
watch the file for changes.

**9.2.5 A hand-edited settings file is not validated.** The save path clamps
the theme and the density (`src/settings.rs:40-58`), but the load path does
not (`src/settings.rs:79-84`), so `theme: "neon"` is applied as it is. Fix:
run the loaded values through the same check.

**9.2.6 The privacy headers differ on the two paths.** The CEF path sends
`DNT: 1` and `Sec-GPC: 1` (`src/shared/simple_handler/mod.rs:2129-2135`), the
proxy path only `DNT: 1` (`src/proxy.rs:365-367`). Fix: send the same
headers on both.

**9.2.7 The chat has one room and no room list.** `EVENT_ROOMS` is declared
but never emitted (`src/chat/mod.rs:113`), the page hard-codes "# general"
(`assets/chat.html:63`), and `ask_before_joining_site_rooms` is a dead field
(`src/chat/settings.rs:23`). The lobby itself is a compiled-in plain-HTTP
relay (`src/chat/config.rs:19-24`) that this repository never deploys, so the
chat only works while that relay answers. Fix: either build the room list and
let the user pick, or take the single room out of the interface and say that
the lobby is a deployment that has to exist.

**9.2.8 The age gate races.** The page sends `confirm_age` and `enable` as
two tasks 150 ms apart (`assets/chat.html:233`) on a runtime with two workers
(`src/chat/mod.rs:37-46`), so `enable` can arrive before the age is stored
and fail. Fix: make the confirmation and the enabling one task.

**9.2.9 The Windows installer still says 0.1.0.**
`assets/windows/installer.nsi:38` hard-codes the version while `Cargo.toml`
says 0.1.2. Fix: take it from the manifest, as the Linux package does.

**9.2.10 The macOS menu bar has no chat entry.** The macOS menu registers only
the settings (`src/mac/mod.rs:96-99`), while the other platforms' menus have a
chat entry (`src/menu.rs:118`). Fix: add it.

### 9.3 Paths that can lose a navigation

**9.3.1 A popup can lose its navigation.** The popup handler asks the chrome
for a tab before CEF has given the browser its id, and the resolved URL is then
dropped (`src/shared/simple_handler/mod.rs:726-732,1176-1188,3016-3029`).
Fix: hold the URL until the browser is ready. Acceptance: a journey that opens
a `window.open` and checks that the new tab shows the page.

**9.3.2 A bare name can skip the resolver.** The startup page and the home URL
are loaded directly (`src/shared/simple_app.rs:435-458`), `open_tab` loads its
URL directly (`src/shared/simple_handler/mod.rs:2461`), and the resolver's own
error is discarded (`src/shared/simple_handler/mod.rs:2684`), which lets a
`javascript:` URL reach `load_url`. Fix: send the startup and home URLs
through the resolver as well, and refuse a URL the resolver rejects.

**9.3.3 The resolver drops the port.** `example.com:8080` becomes
`https://example.com/` (`core/src/resolver.rs:241-249`). Fix: keep the port.

**9.3.4 A failed subresource writes the error page.** `on_load_error` does not
check for the main frame (`src/shared/simple_handler/mod.rs:1369-1374`), so a
missing image can replace the page with the error page. Fix: only the main
frame.

**9.3.5 The external button can hand out a loopback address.** The remembered
choice stores the proxy URL (`src/shared/simple_handler/mod.rs:1398`), and the
external command prefers it (`src/shared/simple_handler/mod.rs:2539-2545`), so
the system browser gets `http://<name>.localhost:<port>/` — which only works
while the app runs. Fix: store the public address for the external command.

**9.3.6 A download lands in the wrong place on Windows.** The path is built
from `$HOME` (`src/shared/simple_handler/mod.rs:1698-1704`), which is normally
not set there. Fix: use the known folder.

**9.3.7 The remembered choice can cut the unavailable page short.** The choice
is checked before the resolver (`src/shared/simple_handler/mod.rs:2632`), so
the "Try again" of the unavailable page can be a no-op. Fix: the retry clears
the choice.

### 9.4 Features that are missing

- Per-tab favicons (there is a source dot instead).
- A tab count, and a scroll-into-view for the selected tab when the strip
  overflows.
- The page title in the address bar (only the tab shows it).
- The MSI and AppImage installers.
- A production `fdn-chatd` lobby.

### 9.5 What the journeys do not reach yet

- The unavailable page (the registry cannot be made unreachable without a way
  to point the app at another registry base).
- The external open button and the review buttons.
- The downloads.
- The popup to tab.
- The native keyboard path of a content page (the journeys send key events
  into the chrome page; the content handler needs a real key).
- The density and the custom CSS (the journeys cover the theme only).
- The chat's own features: sending, the profile view, the ignore list, the
  clear data, and the per-tab enablement.

### 9.6 Dead code to remove or wire up

`src/chat/mod.rs:113` (`EVENT_ROOMS`), `:258` (`active_tab`), `:363`
(`panel_enabled`), `:731` (`set_active_tab`); `src/chat/settings.rs:23`;
`src/chat/transport.rs:72,76,80,84,111`; `src/chat/identity.rs:79`;
`src/chat/policy.rs:76,196`; `src/chat/protocol.rs:48` (`reply_to`);
`src/chat/room.rs:9`; `core/src/resolver.rs:45,143,151,161,307,315,323,447`;
`core/src/registry.rs:159` (`RegistryClient::cached`); `core/src/lib.rs:6`.
The resolver's own decision memory is never used: a fresh `Navigator` is built
for every navigation (`src/shared/simple_handler/mod.rs:2557,2657`), so the
app's own static map (`src/shared/simple_handler/mod.rs:322-337`) is the only
memory that works.

## Suggested order

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9.

Inside Phase 9: 9.1 first (the user sees them), then 9.2, then 9.3, then 9.4
and 9.5. Each fix comes with the journey that holds it down.

## Definition of done

A signed macOS `.app` that launches its start page and frame-blocking sites,
keeps the address bar fixed, supports independent tabs, resolves free-domain
names through the registry with conflict/claim flows, persists settings, and
runs the chat backend — with repeatable tests and screenshots produced without
opening windows, and no defect a user can see in the list of Phase 9.

## Developer setup

CEF is provisioned by Cargo on all desktop platforms: `.cargo/config.toml`
defaults `CEF_PATH` to `./cef-binary`, and the vendored `download-cef` patch
raises the CDN index limit so a clean `cargo run` can download the pinned
156.0.26 archive. Override `CEF_PATH` to use an existing distribution.
