#!/bin/sh
# Download the exact CEF build this project links against (x86_64 Linux).
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST=${1:-"$ROOT/cef-binary"}
VERSION=156.0.26+g8ea0c73+chromium-156.0.8078.88
NAME="cef_binary_${VERSION}_linux64"
URL="https://cef-builds.spotifycdn.com/${NAME}.tar.bz2"

if [ "$(uname -s)" != "Linux" ] || [ "$(uname -m)" != "x86_64" ]; then
  echo "this helper downloads the x86_64 Linux CEF distribution only" >&2
  exit 1
fi
mkdir -p "$DEST"
TMP=$(mktemp)
trap 'rm -f "$TMP"' EXIT
echo "downloading $URL"
curl -fL --progress-bar "$URL" -o "$TMP"
tar -xjf "$TMP" -C "$DEST" --strip-components=1
test -f "$DEST/libcef.so"
echo "CEF ready at $DEST"
