#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
ENGINE=${ENGINE:-podman}
mkdir -p target/docker-out
"$ENGINE" build -t free-domain-browser-cef-web-test -f Dockerfile.test .
"$ENGINE" run --rm \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1000,700 --screenshot=/out/settings.png file:///assets/settings.html
echo "screenshots: $ROOT/target/docker-out/chrome.png $ROOT/target/docker-out/settings.png"
