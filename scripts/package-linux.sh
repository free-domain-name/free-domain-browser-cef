#!/bin/sh
# Build a portable Linux x64 package from the CEF bootstrap output.
# The package is a tar.gz containing the browser, helper, CEF runtime and a
# desktop entry. It does not need root to run.
set -eu
cd "$(dirname "$0")/.."
if [ "${FDB_SKIP_BUILD:-0}" != "1" ]; then
  cargo run --release --bin bundle -- --build-only --release
fi
ARCH="$(uname -m)"
NAME="FreeDomainBrowser-CEF-linux-${ARCH}"
ROOT="target/package"
STAGE="$ROOT/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -R target/bootstrap/release/. "$STAGE/"
rm -f "$STAGE"/*.d
install -m 644 assets/icon.png "$STAGE/free-domain-browser-cef.png"
install -m 644 LICENSE "$STAGE/LICENSE"
cat > "$STAGE/free-domain-browser-cef.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Free Domain Browser
Comment=Browse free domains with registry-first resolution
Exec=free-domain-browser-cef %u
Icon=free-domain-browser-cef
Terminal=false
Categories=Network;WebBrowser;
MimeType=x-scheme-handler/fdn;x-scheme-handler/http;x-scheme-handler/https;
DESKTOP
chmod 755 "$STAGE/free-domain-browser-cef" "$STAGE/free_domain_browser_cef_helper" 2>/dev/null || true
OUT="target/$NAME.tar.gz"
rm -f "$OUT"
tar -C "$ROOT" -czf "$OUT" "$NAME"
echo "wrote $OUT"
