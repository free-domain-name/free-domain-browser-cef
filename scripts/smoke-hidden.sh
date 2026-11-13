#!/bin/bash
# Headless smoke test: start the CEF bundle hidden, wait for DevTools, list targets.
set -u
cd "$(dirname "$0")/.."
PORT="${FDB_SMOKE_PORT:-9222}"
LOG="${TMPDIR:-/tmp}/free-domain-browser-cef-smoke.log"
rm -f "$LOG"
cargo run -- --initial-show-state=hidden --remote-debugging-port="$PORT" >"$LOG" 2>&1 &
PID=$!
cleanup() {
  kill "$PID" 2>/dev/null || true
  pkill -f 'free-domain-browser-cef.app/Contents/MacOS/free-domain-browser-cef' 2>/dev/null || true
  pkill -f 'free_domain_browser_cef_helper' 2>/dev/null || true
}
trap cleanup EXIT
for _ in $(seq 1 120); do
  if curl -sf --max-time 1 "http://127.0.0.1:$PORT/json/version" >/dev/null 2>&1; then
    echo "CEF DevTools ready on $PORT"
    curl -s --max-time 3 "http://127.0.0.1:$PORT/json" |
      python3 -c 'import json,sys; targets=json.load(sys.stdin); print(f"{len(targets)} targets"); [print(t.get("url","")[:80]) for t in targets]'
    if grep -q "DevTools listening" "$LOG"; then echo "log: DevTools listening"; else echo "missing DevTools log"; exit 1; fi
    exit 0
  fi
  sleep 1
done
echo "timed out waiting for CEF; last log:" >&2
tail -20 "$LOG" >&2
exit 1
