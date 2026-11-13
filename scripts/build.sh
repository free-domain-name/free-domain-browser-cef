#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
: "${CEF_PATH:?set CEF_PATH to a CEF 156.0.26 distribution}"
export CEF_PATH
OS=$(uname -s)
if [ "$OS" = "Linux" ]; then
  if [ ! -f "$CEF_PATH/libcef.so" ]; then
    echo "CEF_PATH does not look like a linux64 CEF distribution: $CEF_PATH" >&2
    exit 1
  fi
  LD_LIBRARY_PATH="$CEF_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export LD_LIBRARY_PATH
else
  export DYLD_FALLBACK_LIBRARY_PATH="$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
fi
cargo build
cargo run --bin bundle -- --build-only
