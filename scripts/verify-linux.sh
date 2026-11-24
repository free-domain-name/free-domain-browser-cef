#!/bin/bash
# Run the verification journeys against the Linux build, on a virtual display.
#
# It runs inside the container of Dockerfile.linux:
#
#   docker run --rm --shm-size=1g --entrypoint /usr/local/bin/verify-linux.sh fdb-cef-linux
#
# The journeys drive the real browser through the DevTools protocol, so this
# is the same check the macOS runner does, on the Linux build. The browser
# needs an X display on Linux even when it opens no window, so the script
# starts an Xvfb display and the journeys run against it.
set -eu
cd "${FDB_DIR:-/app}"

export CEF_PATH="${CEF_PATH:-/app/cef-binary}"
export DISPLAY="${DISPLAY:-:99}"

# The runner speaks the DevTools protocol over a WebSocket. Node ships one
# from version 22 on, and the Debian package is older than that, so a current
# Node is unpacked when the image does not have a usable one.
NODE_VERSION="${FDB_NODE_VERSION:-v22.14.0}"
if ! node -e 'process.exit(typeof WebSocket === "function" ? 0 : 1)' >/dev/null 2>&1; then
  case "$(uname -m)" in
    x86_64) node_arch=x64 ;;
    aarch64 | arm64) node_arch=arm64 ;;
    *)
      echo "no Node build for $(uname -m)" >&2
      exit 1
      ;;
  esac
  echo "installing Node $NODE_VERSION ($node_arch)"
  curl -fsSL "https://nodejs.org/dist/$NODE_VERSION/node-$NODE_VERSION-linux-$node_arch.tar.gz" |
    tar -xz -C /usr/local --strip-components=1
fi
node --version

# The CEF runtime is not next to the binary on Linux: the loader has to be
# pointed at the directory that holds libcef.so.
lib_dir=""
for candidate in "$CEF_PATH" "$CEF_PATH"/* "$CEF_PATH"/*/*; do
  if [ -f "$candidate/libcef.so" ]; then
    lib_dir="$candidate"
    break
  fi
done
if [ -z "$lib_dir" ]; then
  echo "libcef.so not found below $CEF_PATH" >&2
  exit 1
fi
export LD_LIBRARY_PATH="$lib_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
echo "CEF runtime: $lib_dir"

# Build the browser unless the image already carries it. The pages are
# compiled into the binary, so a changed page means a new build.
if [ "${FDB_SKIP_BUILD:-0}" != "1" ]; then
  # Cargo decides with the timestamps of the files. A tree that was copied
  # into an image can be older than the build in it, so the sources are
  # touched to make sure the rebuild happens.
  find src core assets -type f -exec touch {} +
  cargo build -j "${FDB_BUILD_JOBS:-2}" --bin free-domain-browser-cef
fi

if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  Xvfb "$DISPLAY" -screen 0 1400x900x24 >/dev/null 2>&1 &
  xvfb_pid=$!
  trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
  for _ in $(seq 1 50); do
    xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep 0.2
  done
fi
if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  echo "Xvfb did not start on $DISPLAY" >&2
  exit 1
fi

node verification/run.mjs "$@"
