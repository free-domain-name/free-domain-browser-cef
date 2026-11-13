#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CEF_PATH=${CEF_PATH:-/tmp/cef-binary-156.0.26}
CMAKE=${CMAKE:-/tmp/buildtools/cmake/data/bin/cmake}
NINJA_DIR=$(dirname "${CMAKE_MAKE_PROGRAM:-/tmp/buildtools/bin/ninja}")
mkdir -p "$ROOT/.cargo"
cat > "$ROOT/.cargo/config.toml" <<EOF
[env]
CEF_PATH = "$CEF_PATH"
CMAKE = "$CMAKE"
PATH = { value = "$NINJA_DIR:/Users/ozkan/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin", force = true }
EOF
echo "wrote $ROOT/.cargo/config.toml"
