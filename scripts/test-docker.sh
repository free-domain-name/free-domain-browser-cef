#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
ENGINE=${ENGINE:-docker}
mkdir -p target/docker-out
"$ENGINE" build -t free-domain-browser-cef-web-test -f Dockerfile.test .
# The chrome template contains a build-time placeholder; substitute it before
# loading the page in Chromium so its JavaScript actually runs.
sed 's|__FDB_INITIAL_URL__|"https://example.com/"|' assets/chrome.html > target/docker-out/chrome.html
"$ENGINE" run --rm \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1000,820 --screenshot=/out/settings.png file:///assets/settings.html
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1000,900 --screenshot=/out/chat.png file:///assets/chat.html
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=420,760 --screenshot=/out/chat-panel.png 'file:///assets/chat.html?panel=1'
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars \
  --window-size=1200,760 --screenshot=/out/newtab.png file:///assets/newtab.html
# Dynamic smoke checks: JS must render the tab strip/menu, settings must expose
# the theme picker, and chat must expose the profile editor and age gate.
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/target/docker-out:/out" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars --virtual-time-budget=2000 --dump-dom file:///out/chrome.html > target/docker-out/chrome.dom
grep -q 'id="menu"' target/docker-out/chrome.dom
grep -q 'New tab (Cmd/Ctrl+T)' target/docker-out/chrome.dom
# The persistent status bar was replaced by a transient bubble, and the
# resolution source is a per-tab dot rather than its own element.
grep -q 'id="statusBubble"' target/docker-out/chrome.dom
grep -q 'data-source="' target/docker-out/chrome.dom
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars --virtual-time-budget=2000 --dump-dom file:///assets/settings.html > target/docker-out/settings.dom
grep -q 'id="theme"' target/docker-out/settings.dom
grep -q 'id="density"' target/docker-out/settings.dom
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars --virtual-time-budget=2000 --dump-dom file:///assets/chat.html > target/docker-out/chat.dom
grep -q 'id="profileDescription"' target/docker-out/chat.dom
grep -q 'id="age"' target/docker-out/chat.dom
grep -q 'id="clearData"' target/docker-out/chat.dom
grep -q 'id="count"' target/docker-out/chat.dom
grep -q 'id="panelClose"' target/docker-out/chat.dom
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars --virtual-time-budget=2000 --dump-dom 'file:///assets/chat.html?panel=1' > target/docker-out/chat-panel.dom
grep -q 'class="panel"' target/docker-out/chat-panel.dom
grep -q 'id="ignoreList"' target/docker-out/chat.dom
"$ENGINE" run --rm --entrypoint chromium \
  -v "$ROOT/assets:/assets:ro" \
  free-domain-browser-cef-web-test \
  --headless --no-sandbox --disable-gpu --hide-scrollbars --virtual-time-budget=2000 --dump-dom file:///assets/newtab.html > target/docker-out/newtab.dom
grep -q 'data-url="fdn://google.com"' target/docker-out/newtab.dom
echo "web checks passed"
echo "screenshots: $ROOT/target/docker-out/{chrome,settings,chat,chat-panel,newtab}.png"
