#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
CEF_PATH=${CEF_PATH:-"$ROOT/cef-binary"}
export CEF_PATH
OS=$(uname -s)
if [ "$OS" = "Linux" ]; then
  LD_LIBRARY_PATH="$CEF_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export LD_LIBRARY_PATH
else
  export DYLD_FALLBACK_LIBRARY_PATH="$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
fi
missing=
for tool in cargo cmake ninja; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
  echo "missing build tools:$missing" >&2
  exit 1
fi
cargo build
cargo run --bin bundle -- --build-only
