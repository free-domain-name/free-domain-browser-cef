#!/bin/bash
# Smoke-test the portable Linux package: unpack the tarball, run the bundled
# launcher on the virtual display and check that the browser comes up.
#
# This is the check that the release artifact really works: the package has to
# carry the CEF runtime with it.
set -u
cd "${FDB_DIR:-/app}"
PORT="${FDB_SMOKE_PORT:-9223}"
PACKAGE="${FDB_PACKAGE:-$(ls -1 target/FreeDomainBrowser-CEF-linux-*.tar.gz 2>/dev/null | head -1)}"
WORK="$(mktemp -d)"
LOG="${TMPDIR:-/tmp}/fdb-cef-linux-package-smoke.log"
rm -f "$LOG"

if [ -z "$PACKAGE" ] || [ ! -f "$PACKAGE" ]; then
  echo "no Linux package found; run scripts/package-linux.sh first" >&2
  exit 1
fi
echo "package: $PACKAGE"
tar -C "$WORK" -xzf "$PACKAGE"
STAGE="$(find "$WORK" -maxdepth 1 -mindepth 1 -type d | head -1)"
if [ -z "$STAGE" ]; then
  echo "the package is empty" >&2
  exit 1
fi

if [ ! -f "$STAGE/libcef.so" ]; then
  echo "the package does not carry libcef.so" >&2
  exit 1
fi
if [ ! -x "$STAGE/free-domain-browser-cef" ]; then
  echo "the package has no launcher" >&2
  exit 1
fi
echo "runtime: $STAGE/libcef.so"

export DISPLAY="${DISPLAY:-:99}"
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

"$STAGE/free-domain-browser-cef" \
  --disable-gpu --disable-dev-shm-usage \
  --remote-debugging-port="$PORT" \
  --url="${FDB_SMOKE_URL:-https://example.com/}" >"$LOG" 2>&1 &
APP_PID=$!
cleanup() {
  kill "$APP_PID" 2>/dev/null || true
  pkill -f "$STAGE/free-domain-browser-cef.bin" 2>/dev/null || true
  [ -n "${XVFB_PID:-}" ] && kill "$XVFB_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

for _ in $(seq 1 120); do
  if curl -sf --max-time 1 "http://127.0.0.1:$PORT/json/version" >/dev/null 2>&1; then
    echo "CEF DevTools ready on $PORT"
    curl -s --max-time 3 "http://127.0.0.1:$PORT/json" |
      python3 -c 'import json,sys; targets=json.load(sys.stdin); print(f"{len(targets)} targets"); [print(t.get("url","")[:80]) for t in targets]'
    if ! kill -0 "$APP_PID" 2>/dev/null; then
      echo "the packaged browser exited early; last log:" >&2
      tail -20 "$LOG" >&2
      exit 1
    fi
    echo "the packaged browser is running"
    exit 0
  fi
  sleep 1
done
echo "timed out waiting for the packaged browser; last log:" >&2
tail -20 "$LOG" >&2
exit 1
