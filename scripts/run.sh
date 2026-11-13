#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

OS=$(uname -s)
# Cargo's .cargo/config.toml defaults CEF_PATH to ./cef-binary. If it is empty,
# cef-dll-sys downloads the pinned CEF build automatically on the first build.
CEF_PATH=${CEF_PATH:-"$ROOT/cef-binary"}
export CEF_PATH
if [ ! -f "$CEF_PATH/libcef.so" ] && [ "$OS" = "Linux" ]; then
  echo "CEF is not present at $CEF_PATH yet; cargo will download it now." >&2
fi
if [ "$OS" = "Linux" ]; then
  LD_LIBRARY_PATH="$CEF_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export LD_LIBRARY_PATH
fi

if ! command -v cmake >/dev/null 2>&1 && [ -d /tmp/buildtools/cmake/data/bin ]; then
  PATH=/tmp/buildtools/cmake/data/bin:/tmp/buildtools/bin:$PATH
  export PATH
fi
missing=
for tool in cargo cmake ninja; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
if [ -n "$missing" ]; then
  echo "missing build tools:$missing" >&2
  echo "install CMake and Ninja, then rerun (Debian: apt install cmake ninja-build; macOS: brew install cmake ninja)" >&2
  exit 1
fi

if [ "${1:-}" = "--hidden" ]; then
  shift
  exec cargo run -- --initial-show-state=hidden "$@"
fi
exec cargo run -- "$@"
