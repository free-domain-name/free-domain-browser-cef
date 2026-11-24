# Changelog

All notable changes to the Free Domain Browser CEF build are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The Free Registry is asked first again. The chrome bar added `https://` to
  every name that was typed, and the browser then treated it as a plain web
  address: the registry was never consulted for an ordinary domain, a name
  that is in both the registry and public DNS went straight to the public
  site instead of opening the chooser, and a name nobody registered showed
  the browser error page instead of the page that offers to claim it. The
  chrome hands the name on as it was typed now, the resolver asks the
  registry first and only falls back to the public web when the name is not
  there. The start page opens the conflict sample through `fdn://` as well,
  like its other registry samples.
- Escape in the chat no longer closes the panel while the age gate is in
  front of it. The gate is a fixed overlay, and a fixed element has no
  `offsetParent`, so the check that guarded the panel always believed the
  gate was gone.

### Added

- A verification framework. The journeys in `verification/journeys` are the
  written-down version of what the browser must do, and `scripts/verify.sh`
  checks them against the real build: it starts the application binary,
  drives the running window and reads what is really there. No mock browser
  and no fake network. The journeys run in CI as well, with the browser
  hidden because the runner has no screen.
- One command to install the portable build on macOS, Linux and Windows, on
  the project page and in the README:
  `curl -fsSL https://free-domain-name.github.io/install.sh | sh`. The script
  resolves the newest release, verifies its SHA-256 against `checksums.txt`
  and unpacks it into the home directory.
- A check for the pages under `docs/`, so a broken page is caught before it
  reaches the site.
- The verification journeys now run on Linux and on Windows as well:
  `scripts/verify-linux.sh` runs them inside the container of
  `Dockerfile.linux` on a virtual display, and `scripts/verify-windows.ps1`
  runs them against the packaged build. Both are jobs in the CI workflow.
- Three more journeys: `conflict` holds the chooser down for a name that is
  in the registry and in public DNS, `chat` holds the chat down from the
  age gate to the connected room, and `menu-macos` holds the commands of the
  native macOS menu bar down.

### Changed

- One theme instead of many. The seven skins are gone: the browser now wears
  a single Material theme, in the light and the dark variant, and the settings
  page keeps only the theme, the layout density and the custom CSS. A skin
  saved in an older `settings.json` is ignored, and the browser falls back to
  Material on its own. The chrome and every internal page read their colours
  from the same token layer now, so the light theme looks like the dark one
  instead of a washed-out copy of it. The selected tab is a filled surface
  with an accent underline, and the gear lights up in the accent colour while
  the settings tab is in front.
- The settings page opens in a tab of its own. Pressing the gear (or
  Cmd/Ctrl+, or the Settings entry of the native menu) no longer covers the
  page with a sheet: the settings get a tab in the strip, the page you were
  looking at stays where it is, and switching back to another tab shows that
  tab again. The tab carries the address `fdb://settings`, and the Close
  button and Escape in the settings close that tab.
- The Rust CI is skipped for pushes that only touch the pages or the
  markdown. Those pushes run the cheap page check instead of the five
  browser builds.

### Open

- Build the native MSI and AppImage installers. The Linux `.deb` and the
  Windows NSIS setup are already produced by the packaging scripts.

### Known limitations

- Keep the chat identity in a plain file (`chat-identity.json`) next to the chat
  settings instead of the OS keychain.
- Ship the Linux and Windows builds unsigned. The macOS build is signed with
  the Developer ID and notarized; the other platforms have no signing
  identity set up yet.
- The macOS build runs without Chromium's renderer sandbox. The seatbelt
  sandbox needs the App Sandbox, and an App Sandboxed app cannot register the
  Mach port that Chromium's helper processes rendezvous through. The helper
  processes trap with SIGTRAP on launch without it and the browser quits with
  "GPU process isn't usable".

## [0.1.2] - 2026-11-23

### Fixed

- The macOS release build starts again. Release builds enabled Chromium's
  seatbelt sandbox, which needs the App Sandbox entitlement; the helper
  processes trapped with SIGTRAP on launch and the browser quit with "GPU
  process isn't usable" before showing a window. The macOS build now runs
  without the renderer sandbox, and the release workflow smoke tests the
  signed bundle so a broken signature cannot ship again.

## [0.1.1] - 2026-11-22

### Added

- Sign the macOS release with the Developer ID and notarize it with Apple.
  The release workflow imports the certificate from a secret, signs the
  bundle, staples the ticket and rebuilds the archives, so Gatekeeper reports
  the build as "Notarized Developer ID" and opens without a prompt.
- Sign the nested libraries of the Chromium framework. They came with a linker
  signature only, and the notary rejected the whole archive because of them.

### Fixed

- Find `makensis` where Chocolatey puts it, so the Windows setup executable is
  built instead of being skipped.
- Leave the CEF C++ wrapper sources out of the packages.

## [0.1.0] - 2026-11-22

### Added

- Run the browser in a CEF Views top-level window: a fixed chrome bar on top and
  one page view per tab.
- Add the chrome bar with a tab strip and an address bar, with back, forward,
  reload and home.
- Support the seven skins, the dark, light and system themes, and the compact
  density.
- Add the community chat over iroh-gossip, in a side panel that follows the
  active tab.
- Add the community reviews, opened in a reusable dedicated window.
- Add the settings overlay below the chrome.
- Handle the `fdn://` protocol and resolve the registry names before public DNS.
- Add the error pages with a retry.
- Add the Browser and Edit menus on macOS, Windows and Linux.
- Add the Linux Docker build and its headless smoke test.
- Add the CI lint job with `cargo fmt --check` and `cargo clippy -D warnings`.
- Add the packaging scripts for macOS, Linux and Windows.
- Publish the Linux preview package from CI as a prerelease, with a
  `checksums.txt` next to it.
- Add the page of this build at
  <https://free-domain-name.github.io/free-domain-browser-cef/>, and a section
  on the project home page that puts this build and the stable Tauri build side
  by side.

### Fixed

- Build the browser for Windows at all. The Windows code did not compile: it
  used the browser host trait without importing it and treated the window
  handle as a raw pointer, while it is the `HWND` wrapper around it. The
  release job went green anyway, because PowerShell does not stop on a native
  command that fails, so the packaging script now checks the exit code.
- Stop the native menu bar from freezing the browser on Windows. The menu was
  set on the window from inside the CEF callback that asked for it, which
  deadlocked the UI thread: the browser started, but it never answered on its
  DevTools port. The window procedure is taken over first and the menu is set
  by a message that the window handles in its own message loop.
- Ship the CEF runtime with the Linux and Windows packages. The archives only
  carried the browser binaries, so a user could not start them: the loader
  found no `libcef.so` and Windows found no `libcef.dll`. The packages now
  carry the runtime and its resources, the Linux package starts through a
  launcher that points the loader at it and reports the missing setuid
  sandbox, and the Debian package installs the sandbox helper as root.
- Run the macOS CI job again. The `macos-14` runner image was retired, so the
  `macos-smoke` job was cancelled before it ran a single step and the macOS
  release package was never built. Both workflows use `macos-latest` now, and
  the CEF cache key follows the runner architecture.
- Refresh the docs: the Browser and Edit menus exist on every platform, the
  Linux `.deb` and the Windows NSIS setup are built, and the test count is
  current.

[Unreleased]: https://github.com/free-domain-name/free-domain-browser-cef/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/free-domain-name/free-domain-browser-cef/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/free-domain-name/free-domain-browser-cef/releases/tag/v0.1.0
