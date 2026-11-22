# Changelog

All notable changes to the Free Domain Browser CEF build are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Open

- Build the native MSI and AppImage installers. The Linux `.deb` and the
  Windows NSIS setup are already produced by the packaging scripts.

### Known limitations

- Keep the chat identity in a plain file (`chat-identity.json`) next to the chat
  settings instead of the OS keychain.
- Ship the Linux and Windows builds unsigned. The macOS build is signed with
  the Developer ID and notarized; the other platforms have no signing
  identity set up yet.

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
