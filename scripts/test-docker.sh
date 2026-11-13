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
echo "screenshot: $ROOT/target/docker-out/chrome.png"
