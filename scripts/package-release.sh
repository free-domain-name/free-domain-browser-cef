#!/bin/sh
# Build an unsigned release .app and zip it. Code signing is a separate step.
set -eu
cd "$(dirname "$0")/.."
cargo run --release --bin bundle -- --build-only --release
APP="target/bundle/free-domain-browser-cef.app"
OUT="target/FreeDomainBrowser-CEF.zip"
rm -f "$OUT"
ditto -c -k --keepParent "$APP" "$OUT"
echo "wrote $OUT"
