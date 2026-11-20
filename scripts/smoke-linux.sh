#!/bin/bash
# Headless smoke test on Linux: run the built browser on a virtual display,
# wait for the DevTools endpoint and list the targets.
#
# The hidden show state is macOS only, so on Linux the window opens on the
# Xvfb display. Nothing is shown to a user.
set -u
cd "${FDB_DIR:-/app}"
PORT="${FDB_SMOKE_PORT:-9222}"
BIN="${FDB_BIN:-target/debug/free-domain-browser-cef}"
LOG="${TMPDIR:-/tmp}/fdb-cef-linux-smoke.log"
rm -f "$LOG"

export DISPLAY="${DISPLAY:-:99}"

# The CEF runtime is not next to the binary on Linux: the loader has to be
# pointed at the directory that holds libcef.so. The distribution lives one
# or two levels below CEF_PATH.
cef_lib_dir() {
  local root="${CEF_PATH:-cef-binary}"
  local candidate
  for candidate in "$root" "$root"/* "$root"/*/*; do
    if [ -f "$candidate/libcef.so" ]; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}
if lib_dir=$(cef_lib_dir); then
  export LD_LIBRARY_PATH="$lib_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  echo "CEF runtime: $lib_dir"
else
  echo "libcef.so not found below ${CEF_PATH:-cef-binary}" >&2
  exit 1
fi

if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  Xvfb "$DISPLAY" -screen 0 1400x900x24 >/dev/null 2>&1 &
  XVFB_PID=$!
  for _ in $(seq 1 50); do
    xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep 0.2
  done
fi
if ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  echo "Xvfb did not start on $DISPLAY" >&2
  exit 1
fi

"$BIN" --no-sandbox --disable-gpu --disable-dev-shm-usage \
  --remote-debugging-port="$PORT" \
  --url="${FDB_SMOKE_URL:-https://example.com/}" >"$LOG" 2>&1 &
APP_PID=$!
cleanup() {
  kill "$APP_PID" 2>/dev/null || true
  [ -n "${XVFB_PID:-}" ] && kill "$XVFB_PID" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 120); do
  if curl -sf --max-time 1 "http://127.0.0.1:$PORT/json/version" >/dev/null 2>&1; then
    echo "CEF DevTools ready on $PORT"
    curl -s --max-time 3 "http://127.0.0.1:$PORT/json" |
      python3 -c 'import json,sys; targets=json.load(sys.stdin); print(f"{len(targets)} targets"); [print(t.get("url","")[:80]) for t in targets]'
    if ! kill -0 "$APP_PID" 2>/dev/null; then
      echo "browser exited early; last log:" >&2
      tail -20 "$LOG" >&2
      exit 1
    fi
    echo "browser is running"
    exit 0
  fi
  sleep 1
done
echo "timed out waiting for CEF; last log:" >&2
tail -20 "$LOG" >&2
exit 1
