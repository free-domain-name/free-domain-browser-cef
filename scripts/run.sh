#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

OS=$(uname -s)
CEF_PATH=${CEF_PATH:-"$ROOT/cef-binary"}
if [ ! -d "$CEF_PATH" ] && [ -d /tmp/cef-binary-156.0.26 ]; then
  CEF_PATH=/tmp/cef-binary-156.0.26
fi
if [ "$OS" = "Linux" ]; then
  if [ ! -f "$CEF_PATH/libcef.so" ]; then
    echo "set CEF_PATH to a CEF 156.0.26 linux64 distribution (libcef.so not found)" >&2
    echo "run scripts/fetch-cef-linux.sh first, or download:" >&2
    echo "https://cef-builds.spotifycdn.com/cef_binary_156.0.26+g8ea0c73+chromium-156.0.8078.88_linux64.tar.bz2" >&2
    exit 1
  fi
  LD_LIBRARY_PATH="$CEF_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export LD_LIBRARY_PATH
else
  if [ ! -d "$CEF_PATH" ]; then
    echo "set CEF_PATH to a CEF 156.0.26 macOS distribution" >&2
    exit 1
  fi
fi
export CEF_PATH

if ! command -v cmake >/dev/null 2>&1 && [ -d /tmp/buildtools/cmake/data/bin ]; then
  PATH=/tmp/buildtools/cmake/data/bin:/tmp/buildtools/bin:$PATH
  export PATH
fi

if [ "${1:-}" = "--hidden" ]; then
  shift
  exec cargo run -- --initial-show-state=hidden "$@"
fi
exec cargo run -- "$@"
