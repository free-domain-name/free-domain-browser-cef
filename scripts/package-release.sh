#!/bin/sh
# Build an ad-hoc-signed release .app and zip it. Developer ID signing is separate.
set -eu
cd "$(dirname "$0")/.."
cargo run --release --bin bundle -- --build-only --release
APP="target/bundle/free-domain-browser-cef.app"
OUT="target/FreeDomainBrowser-CEF.zip"
rm -f "$OUT"
ditto -c -k --keepParent "$APP" "$OUT"
echo "wrote $OUT"
if command -v hdiutil >/dev/null 2>&1; then
  DMG="target/FreeDomainBrowser-CEF.dmg"
  rm -f "$DMG"
  if hdiutil create -volname "Free Domain Browser" -srcfolder "$APP" -ov -format UDZO "$DMG"; then
    echo "wrote $DMG"
  else
    echo "warning: DMG creation failed; the .app zip is still available" >&2
  fi
fi
