#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

# CEF on macOS must run from an .app bundle. `cargo run` on the raw binary
# cannot find the framework's ICU resources.
CEF_PATH=${CEF_PATH:-"$ROOT/cef-binary"}
if [ ! -d "$CEF_PATH" ] && [ -d /tmp/cef-binary-156.0.26 ]; then
  CEF_PATH=/tmp/cef-binary-156.0.26
fi
if [ ! -d "$CEF_PATH" ]; then
  echo "set CEF_PATH to a CEF 156.0.26 macOS distribution" >&2
  exit 1
fi
export CEF_PATH
if ! command -v cmake >/dev/null 2>&1 && [ -d /tmp/buildtools/cmake/data/bin ]; then
  PATH=/tmp/buildtools/cmake/data/bin:/tmp/buildtools/bin:$PATH
  export PATH
fi

cargo build --bin free-domain-browser-cef
cargo run --bin bundle

APP="$ROOT/target/bundle/free-domain-browser-cef.app"
if [ "${1:-}" = "--hidden" ]; then
  shift
  exec "$APP/Contents/MacOS/free-domain-browser-cef" --initial-show-state=hidden "$@"
fi
exec open -n "$APP" --args "$@"
