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
