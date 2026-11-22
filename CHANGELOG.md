# Changelog

All notable changes to the Free Domain Browser CEF build are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Run the macOS CI job again. The `macos-14` runner image was retired, so the
  `macos-smoke` job was cancelled before it ran a single step and the macOS
  release package was never built. Both workflows use `macos-latest` now, and
  the CEF cache key follows the runner architecture.
- Refresh the docs: the Browser and Edit menus exist on every platform, the
  Linux `.deb` and the Windows NSIS setup are built, and the test count is
  current.

### Open

- Sign the release builds. Everything published so far is unsigned.
- Notarize the macOS bundle. No Apple credentials are wired into the build.
- Build the native MSI and AppImage installers. The Linux `.deb` and the
  Windows NSIS setup are already produced by the packaging scripts.
- Verify the Windows runtime in CI. Only the Linux container build is verified today.

### Known limitations

- Keep the chat identity in a plain file (`chat-identity.json`) next to the chat
  settings instead of the OS keychain.
- Ship the preview as an unsigned build.

## [0.1.0] - 2026-11-21

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

[Unreleased]: https://github.com/free-domain-name/free-domain-browser-cef/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/free-domain-name/free-domain-browser-cef/releases/tag/v0.1.0
