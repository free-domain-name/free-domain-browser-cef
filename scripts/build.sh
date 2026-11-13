#!/bin/sh
set -eu
: "${CEF_PATH:?set CEF_PATH to a CEF 156.0.26 macOS arm64 distribution}"
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
export DYLD_FALLBACK_LIBRARY_PATH="$CEF_PATH:$CEF_PATH/Chromium Embedded Framework.framework/Libraries"
cargo build
cargo run --bin bundle -- --build-only
