#!/bin/sh
# CEF is provisioned automatically by Cargo into ./cef-binary (see
# .cargo/config.toml and vendor/download-cef/PATCHES.md). This script only
# checks the native build tools needed by CEF's DLL wrapper.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
for tool in cargo cmake ninja; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing $tool; install it (macOS: brew install cmake ninja, Debian: apt install cmake ninja-build)" >&2
    exit 1
  fi
done
echo "build tools OK; run ./scripts/run.sh (CEF downloads on the first build)"
