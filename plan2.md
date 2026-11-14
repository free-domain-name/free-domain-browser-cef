# Plan 2 — parity gaps found by the full Tauri ↔ CEF audit (2026-11-14)

Status: **implemented on 2026-11-14** — items 1–5, 7–12 are done (Rust + page
changes below), item 6 was verified at runtime and needs no code, item 13
remains deferred. See "Implementation status" at the end. Packaging work
(`.app`/DMG/zip/installers) is explicitly **out of scope** throughout.

## How this list was produced

Both apps were inventoried exhaustively with file:line evidence:

- `tari-browser` reference: `ui/main.js` (2409 lines), `ui/index.html`,
  `src-tauri/src/{lib,browser,proxy,registry,resolver,chat/*}.rs`,
  `CHAT_PLAN2.md`. Report headline: Tauri's desktop native-child-webview path is
  **dormant** — `content_mode()` hard-returns `"iframe"` on every platform
  (`src-tauri/src/browser.rs:24-28`), so the shipped Tauri desktop app is an
  iframe-chrome browser with an injected location bridge.
- CEF side: `assets/*.html` (8 pages), `src/shared/simple_handler/mod.rs` (2334
  lines), `simple_app.rs`, `mac/mod.rs`, `src/chat/*` (8 files), `proxy.rs`,
  `settings.rs`, `bin/*`, `PLAN.md`, `MIGRATION.md`.
- `core/{registry,resolver}.rs` are **byte-identical** between the two repos —
  resolver/registry parity is guaranteed by construction, not audited again.

Most parity surfaces check out: chrome bar, dynamic tabs, claim/conflict/
unavailable/error pages (identical `fdn-core`), loopback proxy with header
stripping + cookie sanitization + 64 MiB caps + Host rules, settings store, 7
skins + custom CSS (24k), chat (age gate, profile cards, ignore, clear-data,
keychain identity, bootstrap config), reviews window, downloads (CEF is ahead),
external open, overflow menu, macOS native menus (CEF even adds a Window menu),
and the full keyboard matrix. **CEF is ahead of Tauri** on downloads-with-
progress, reopen tab (Cmd+Shift+T), numeric tab keys, home/cookies/DNT settings,
and DNT/Sec-GPC. The items below are the genuine gaps.

---

## P0 — behavior bugs visible in normal use

### 1. Closing the last tab leaves an empty page area (Tauri opens a fresh tab)
- Tauri: closing the last tab always opens a fresh new tab
  (`tauri-browser/ui/main.js:1390-1408`).
- CEF: `take_tab` sets `active_uid = None` when the tab list becomes empty
  (`src/shared/simple_handler/mod.rs:728-733`), and `close_tab` only switches
  when `active_uid` is `Some` (`mod.rs:1979-1981`) — the result is a window with
  a chrome bar, zero tabs, and a blank content area.
- Fix: when `close_tab` empties the tab list, open a fresh tab (reuse
  `open_tab(inner, next_uid, internal_page_url(NEWTAB_HTML))` and make it
  active).
- Files: `src/shared/simple_handler/mod.rs` (close_tab / take_tab).
- Acceptance: with exactly one tab open, closing it (× / Cmd+W / middle-click)
  immediately shows the new-tab page; repeated close never leaves blank content.

### 2. Cookie semantics are inverted between the two network paths
- The settings checkbox is labeled **"Block third-party cookies"** and is saved
  as `cookies: bool` (`assets/settings.html:33`, `src/settings.rs:8-18`).
- Loopback proxy path: `src/proxy.rs:347-349` drops **all** upstream `Cookie`
  headers when `settings.cookies == false` (i.e. blocks the *most* when blocking
  is *off*) and forwards everything when blocking is *on*.
- CEF request-handler path: `src/shared/simple_handler/mod.rs:1654-1683` strips
  cross-origin cookies when blocking is on — label-correct.
- Tauri behavior (the reference): cookies always pass through both directions,
  with `Domain=`/`Secure`/`Partitioned` sanitized on `Set-Cookie`
  (`tauri-browser/src-tauri/src/proxy.rs:371-385`).
- Fix: every request that reaches the loopback proxy is effectively same-origin
  (pages and rewritten subresources all live under `{domain}.fdn.localhost`), so
  the proxy should **always forward `Cookie`** and keep the existing `Set-Cookie`
  sanitization. Third-party blocking stays the CEF handler's job
  (`on_before_resource_load` cross-origin check) and must be driven by the
  checkbox in that one place. Remove the inverted `!settings.cookies` drop.
- Files: `src/proxy.rs`, unit tests in `src/proxy.rs` (add one asserting the
  Cookie header survives all checkbox states), settings page label stays.
- Acceptance: with **Block third-party cookies** OFF, session cookies work on
  registry sites; with it ON, cross-origin requests carry no `Cookie` while
  same-origin ones still do; both paths agree.

### 3. Unhandled popups spawn stray top-level windows
- `on_before_popup` (`mod.rs:909-933`) blocks and tab-routes only
  `http(s)`/`fdn` popups. Everything else — `about:blank` popups, custom-scheme
  popups, `window.open("")` — falls through to
  `SimpleBrowserViewDelegate::on_popup_browser_view_created`
  (`simple_app.rs:192-210`), which creates a whole new OS window (DevTools
  popups too; `is_devtools` is not filtered).
- Tauri: denies **every** new window and routes all targets through the resolver
  into a tab (`tauri-browser/src-tauri/src/browser.rs:431-441`).
- Fix: in `on_before_popup`, return 1 (block) for **all** targets except when
  `is_devtools` is set; keep the view-delegate window only for DevTools so the
  debug workflow (`--remote-debugging-port`, DevTools popups) still works.
- Files: `src/shared/simple_handler/mod.rs` (on_before_popup).
- Acceptance: a page calling `window.open()` with a blank or unsupported target
  opens nothing; `fdn://`/http targets still become tabs; DevTools still opens.

---

## P1 — Tauri behaviors CEF lacks

### 4. Chat events go to a no-op sink; the panel lags 3 seconds behind
- Tauri pushes `chat://state|message|presence|error` events instantly
  (`tauri-browser/src-tauri/src/chat/mod.rs:245,1016,1019-1037`, consumed at
  `ui/main.js:930-956`).
- CEF: the event sink is `|_,_|` (`src/chat/mod.rs:51`) and the chat page only
  polls `snapshot` every 3 s (`assets/chat.html:185`). Connection-state changes,
  arrivals and presence lag up to 3 s.
- Fix: implement the sink — on any `ChatEvent`, post a
  `fdbChatEvent({kind, payload})` script into the chat page's frame (a task like
  the existing `ChatResponse` poster, `mod.rs:2189-2213`), targeted at the
  browser id whose room produced it. Keep the 3 s poll as a fallback. Update the
  chat page to apply events immediately.
- Files: `src/chat/mod.rs` (service + sink), `src/shared/simple_handler/mod.rs`
  (chat page lookup/posting), `assets/chat.html` (event handling).
- Acceptance: with two peers, a message/presence change appears without waiting
  for the next poll tick; the connecting/connected pill reacts immediately.

### 5. The address bar can be overwritten while the user is typing
- Tauri guards updates: the address is only rewritten when the input is not
  focused (`tauri-browser/ui/main.js:1944-1947`).
- CEF: `render()` unconditionally sets `url.value = current().url`
  (`assets/chrome.html:87`) on every `fdbUpdate`/`fdbLoad` — a slow load or a
  title update can clobber text the user is mid-typing.
- Fix: in `render()` (and `fdbUpdate`/`fdbLoad`), skip the address-bar value
  assignment when `document.activeElement` is the address input.
- Files: `assets/chrome.html`.
- Acceptance: typing "hello.fdn" while a tab finishes loading keeps the typed
  text; the bar still updates after blur.

### 6. Verify in-page URL sync (pushState/hashchange) — add a location bridge if needed
- Tauri tracks in-page navigation inside proxied pages by injecting a bridge
  script that reports href+title on load/popstate/hashchange/click and wraps
  `pushState`/`replaceState` (`tauri-browser/src-tauri/src/proxy.rs:38-73,
  416-425`).
- CEF relies on `on_address_change` (`mod.rs:1320-1332`). Whether CEF 156 fires
  it for **same-document** navigations (pushState/hash) needs a runtime check;
  the tab-strip title side is covered by `on_document_title_changed`.
- Fix: first verify with the hidden smoke harness (load a pushState-using page
  through the proxy; assert chrome receives `fdbUpdate` with the new URL). If
  not observed, inject the same tiny bridge (no-iframe variant: `postMessage`
  is not needed — post straight to the chrome via the `fdb:` console channel)
  from `rewrite_html` and route it in `on_console_message`.
- Files: `src/proxy.rs` (inject), `src/shared/simple_handler/mod.rs` (route),
  `scripts/smoke-hidden.sh` or a devtools check for verification.
- Acceptance: navigating an SPA route (pushState) inside a proxied page updates
  the address bar and the status URL exactly like Tauri.

### 7. Home shows the configured URL; Tauri shows the start page
- Tauri: home button → `showStart()` — resets the current tab to the start page
  (`tauri-browser/ui/main.js:2044, 1610-1622`).
- CEF: home → loads `settings.home` (`mod.rs:1400-1402`), default
  `https://yahoo.com/`.
- Fix (parity call): make the home button load the new-tab page
  (`internal_page_url(NEWTAB_HTML)`) for the current tab; keep the `home`
  setting exclusively as the startup URL. (Decision note: this is the only
  Tauri-divergent behavior found deliberately; if you prefer keeping
  `settings.home`, delete this item instead.)
- Files: `src/shared/simple_handler/mod.rs` (Home action).
- Acceptance: ⌂ with an active registry tab shows the new-tab page and updates
  the address bar to the typed-name state.

### 8. Conflict page offers no cancel
- Tauri's conflict modal has Cancel (backdrop, Escape, Cancel link) that leaves
  the current page untouched (`tauri-browser/ui/index.html:623`,
  `ui/main.js:2211, 2252-2254`).
- CEF: `assets/conflict.html` only offers "Open registry site" / "Open public
  website" — the user is forced to choose or press Back manually.
- Fix: add a "Cancel — decide later" link that returns to the previous state
  (Go Back via bridge, falling back to the new-tab page) and remembers nothing.
- Files: `assets/conflict.html` (+ tiny router branch if a new command is
  needed; `back` already exists).
- Acceptance: opening `google.com` shows the chooser; Cancel leaves the browser
  on the prior page and no choice is memorized for the session.

### 9. Tab strip lacks the per-tab resolution-source dot
- Tauri: every tab shows a colored source dot (registry teal / public blue /
  conflict amber / unregistered) (`tauri-browser/ui/index.html:21-26`,
  `styles.css:169-186`).
- CEF: tabs show title + close only (`assets/chrome.html:74-94`).
- Fix: derive the source from the tab's display URL prefix (same rules as
  `setSource()`), render a dot element per tab, and update it in `fdbUpdate`.
- Files: `assets/chrome.html`.
- Acceptance: a tab on `fdn://hello.fdn` shows the registry dot; a public tab
  shows the public dot; visual check via hidden screenshot.

---

## P2 — plan accuracy, doc fixes, dead code, minor behavior

### 10. Repair PLAN.md / MIGRATION.md claims that do not match the code
- `PLAN.md:22` says "settings persist in localStorage" — the Rust store is
  authoritative (`src/settings.rs`); fix the sentence.
- Phase 5 item "Move the remaining appearance fields off localStorage" is only
  partial: `assets/settings.html:42,49` still read/write a parallel
  `localStorage['fdb-settings']` copy — finish the move (Rust store only) or
  reword the claim. Prefer finishing: delete the localStorage read/write, keep
  the Rust store as the single source.
- Phase 5 "Settings drive the loopback proxy cookie/DNT/cache behavior" — no
  proxy HTTP cache exists anywhere; only the registry cache is cleared
  (`proxy.rs:166-185`). Reword to "cookie/DNT behavior" or delete "cache".
- Phase 6 "per-room localStorage history" — absent (in-memory 200-message ring,
  `src/chat/mod.rs:93,1017-1026`). This **matches Tauri v1** (session-only
  history), so correct the claim rather than implement persistence, unless you
  deliberately want persistence beyond Tauri.
- `MIGRATION.md` parity table: "Reopen tab / numeric tab keys | Tauri: yes" is
  wrong — Tauri has neither (`tauri-browser/ui/main.js` has no reopen or digit
  handling); CEF is ahead. Also note in the "Tabs / history" row that Tauri's
  desktop webview path is currently dormant (iframe-only), so the CEF native-
  view port is the *only* implementation of that architecture.
- Acceptance: a doc-only commit; no code changes except item 10's localStorage
  cleanup.

### 11. Remove dead code found by the audit
- `BridgeAction::Page` — declared (`mod.rs:32`) and matched (`mod.rs:1363`) but
  never constructed. Remove both.
- `STUMBLE_URL` in `assets/chrome.html:59` is computed and never used. Remove.
- `close_chat_panel` bridge command is parsed (`mod.rs:1074, 1383-1387`) but no
  page sends it (chat.html uses the `fdbchat: close_panel` channel). Either
  remove the `fdb:` variant or wire `chat.html`'s close button to it; removing
  is simpler — the `fdbchat:` path already works.

### 12. F5 keyboard shortcut fires regardless of modifiers
- The content KeyboardHandler reloads on VK 116 with any modifier state
  (`mod.rs:1524-1525`), so ⌘F5 / Ctrl+F5 / Shift+F5 all reload (⌘F5 on macOS
  typically toggles VoiceOver-adjacent behaviors or is system-reserved).
- Tauri has no F5 handler at all; CEF keeping plain F5 is a nice extra.
- Fix: require an empty modifier mask for F5.
- Files: `src/shared/simple_handler/mod.rs` (keyboard handler).
- Acceptance: plain F5 reloads; ⌘F5 does nothing.

---

## P3 — optional / stretch (Tauri parity on other platforms)

### 13. Native Browser/Edit menus on Windows and Linux
- Tauri installs the Browser + Edit menu on **every desktop platform**
  (`tauri-browser/src-tauri/src/browser.rs:225-340`).
- CEF: native menus only on macOS (`src/mac/mod.rs`); `win.rs`/`linux.rs` are
  no-op stubs and those platforms use the overflow menu + the keyboard handler.
- This is the largest remaining parity gap and needs platform work (GTK menus /
  Win32 menu bar, or embedding the chrome's overflow menu as a CEF Views popup).
- Recommendation: defer unless Linux/Windows are becoming first-class targets;
  the overflow menu + full keyboard coverage keeps those platforms usable today.

---

## Intentional differences — recorded, no action

- **Window title**: CEF follows the page title of the active tab
  (`mod.rs:382-406`); Tauri pins `Free Domain Browser {version}`. CEF's
  behavior is the conventional-browser one; the version still exists via
  the About toast.
- **Content architecture**: CEF renders tabs as real native BrowserViews (that
  is the point of the port); Tauri desktop ships iframes with an injected
  location bridge. Only affects how item 6 (location bridge) gets implemented.
- **CEF is deliberately ahead of Tauri on**: automatic download saving with
  progress, reopen tab, numeric tab keys, ⌂ home URL setting, third-party
  cookie + DNT/Sec-GPC toggles, security indicator, overflow menu, Window menu,
  DDG search box on the new-tab page, registry-info card in settings, and the
  hideable status bar. None of these need changes.
- **Not portable / not in scope**: Android (mobile) support, Tauri's
  "Browser preview" degraded mode, and Tauri's dormant `content_*` command
  plumbing.
- **Status bar** (since 2026-11-14): CEF intentionally drops the persistent
  status bar that Tauri keeps. Hover/engine status appears in a transient
  bottom-left bubble (modern Chrome/Firefox behavior); provenance lives in
  the per-tab source dots. See item 15 below.

## Suggested implementation order

1 → 2 → 3 (P0 correctness) → 5 → 9 → 8 → 7 (small UI parity) → 4 (chat push)
→ 6 (verify-then-bridge) → 10 → 11 → 12 (cleanups) → 13 (stretch).

Every item needs either a `cargo test` addition, an existing-suite pass, or a
hidden-screenshot/CDP check per the PLAN constraints; no `.app`/DMG/zip builds
are required for any of them.

---

## Implementation status (2026-11-14)

- **Item 1 — done.** `close_tab` now spawns a fresh tab through the chrome
  (`fdbNewTab`) when the tab list empties, guarded by `!is_closing` so the quit
  path is unaffected. The chrome's own last-tab guard (`closeTab` sends
  `home`) also now shows the new-tab page thanks to item 7.
- **Item 2 — done.** Proxy `Cookie` inversion removed (`proxy.rs`); a new test
  `cookies_are_never_stripped_by_the_proxy` pins `cookie`/`set-cookie` outside
  the strip lists. The CEF handler keeps third-party-only blocking.
- **Item 3 — done.** `on_before_popup` denies everything except http(s)/fdn
  (routed to tabs) and `devtools://` (existing view-delegate window preserved).
- **Item 4 — done.** Chat sink installed on first chat command
  (`install_chat_sink` + `ChatEventTask` posts `fdbChatEvent` to the panel and
  every chat tab); `chat.html` applies events instantly (`refresh()` +
  error display), 3 s poll kept as fallback. `AppHandle` callback is now
  swap-at-runtime (`set_callback`, manual `Clone`).
- **Item 5 — done.** `chrome.html` render no longer overwrites the URL field
  while it holds focus; the background-tab lock-icon clobber in `fdbUpdate`
  was fixed along the way.
- **Item 6 — verified, no code needed.** Runtime probe (hidden CEF +
  DevTools CDP, `/tmp` harness): loading
  `http://127.0.0.1:8099/`, which runs `history.pushState({}, '', '/pushed-xyz')`
  1.2 s after load, showed the address bar AND the tab model already carrying
  `…/pushed-xyz` with no document reload (a reload would have shown a
  double-pushed path — it did not). `on_address_change` covers same-document
  navigations in CEF 156; the Tauri-style injected location bridge is
  unnecessary.
- **Item 7 — done.** Home shows the new-tab page and clears the tab's URL in
  the chrome (empty-`fdbUpdate` update); the `home` setting stays the startup
  URL. `fdbUpdate` now accepts an empty-string URL for that reset.
- **Item 8 — done.** `conflict.html` gained "Cancel — decide later" which
  bridges `back` (trusted internal page) to the tab's history.
- **Item 9 — done.** Tab strip renders a per-tab source dot (registry teal /
  public blue / idle muted) derived from the same prefix rules as
  `setSource()`.
- **Item 10 — done.** `settings.html` no longer reads/writes the parallel
  `localStorage['fdb-settings']` copy (Rust store hydrates via query params).
  PLAN.md claims corrected (settings store, cookie/DNT wording without the
  nonexistent proxy cache, session-only chat history); MIGRATION.md rows fixed
  (Tauri lacks reopen/numeric keys — now "CEF ahead"; dormant webview note).
- **Item 11 — done.** Removed `BridgeAction::Page`, the dead `fdb:`
  `close_chat_panel` command + `CloseChatPanel` variant, and chrome's unused
  `STUMBLE_URL`.
- **Item 12 — done.** F5 now requires an empty modifier mask.
- **Item 13 — deferred** (P3 stretch; overflow menu + full key matrix cover
  Linux/Windows today).
- **Verification:** `cargo check` clean, 29/29 unit tests pass, all four
  modified page scripts pass `node --check`, hidden CDP smoke run green.

- **Item 15 (post-plan2 decision, 2026-11-14) — status bar removed, transient
  bubble added.** The user chose the modern-browser model over Tauri's
  persistent bar: `#statusbar` (source pill + URL text + "CEF" mode) and the
  `show_status` setting are gone; hover targets, engine statuses and toasts
  now appear in a `#statusBubble` at the bottom-left that hides itself ~3 s
  after the last update (`chrome.html`; `on_status_message` → `fdbStatus`
  wiring unchanged). The source pill was already redundant after item 9's
  per-tab dots. Changes: `chrome.html` (bubble + removals), `settings.html`
  (toggle row removed), `settings.rs` (`show_status` field dropped —
  `#[serde(default)]` keeps old settings.json files loading), `mod.rs`
  (overlay query params + `fdbSettings` payload). Verified live via CDP:
  `STATUSBAR_GONE=true BUBBLE_SHOWN=true BUBBLE_HIDES=true`, 29/29 tests.
  Recorded as an intentional divergence from Tauri in MIGRATION.md.

- **Item 15 follow-up — reserved strip reclaimed.** Removing the bar left
  `CHROME_HEIGHT` at 116 (chrome layout had been 32 tab strip + 64 toolbar +
  20 status bar), so the content view still started 20 px too low and a dead
  strip painted in chrome-background color sat between the toolbar and the
  webview (user-reported, with screenshot). Both `CHROME_HEIGHT` constants
  (simple_handler/mod.rs:63, simple_app.rs:8) are now 96 with comments; the
  chat panel, settings overlay, and all popup chrome views key off the same
  constant, so everything moved up together. CDP probe of a hidden run:
  chrome view `innerHeight=96`, content view `innerHeight=724`, sum = 820 =
  window height — the webview now sticks directly to the address bar.

