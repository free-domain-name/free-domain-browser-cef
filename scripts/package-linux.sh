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

# Build a Debian package from the same portable tree when dpkg-deb is
# available. This is the closest CEF equivalent to Tauri's .deb bundle.
case "$ARCH" in
  x86_64) DEBARCH=amd64 ;;
  aarch64|arm64) DEBARCH=arm64 ;;
  *) DEBARCH="$ARCH" ;;
esac
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
PKG=free-domain-browser-cef
DEBROOT="$ROOT/deb-$NAME"
rm -rf "$DEBROOT"
mkdir -p "$DEBROOT/DEBIAN" "$DEBROOT/usr/lib/$PKG" "$DEBROOT/usr/bin" \
  "$DEBROOT/usr/share/applications" "$DEBROOT/usr/share/icons/hicolor/512x512/apps" \
  "$DEBROOT/usr/share/doc/$PKG"
cp -R "$STAGE"/. "$DEBROOT/usr/lib/$PKG/"
install -m 644 assets/icon.png "$DEBROOT/usr/share/icons/hicolor/512x512/apps/$PKG.png"
install -m 644 "$STAGE/free-domain-browser-cef.desktop" "$DEBROOT/usr/share/applications/$PKG.desktop"
install -m 644 LICENSE "$DEBROOT/usr/share/doc/$PKG/copyright"
cat > "$DEBROOT/usr/bin/$PKG" <<'WRAPPER'
#!/bin/sh
exec /usr/lib/free-domain-browser-cef/free-domain-browser-cef "$@"
WRAPPER
chmod 755 "$DEBROOT/usr/bin/$PKG"
cat > "$DEBROOT/DEBIAN/control" <<CONTROL
Package: $PKG
Version: $VERSION
Section: net
Priority: optional
Architecture: $DEBARCH
Maintainer: Free Domain Browser contributors <noreply@free-domain-name.github.io>
Depends: libgtk-3-0, libnss3, libgbm1, libasound2
Description: Browse free domains with registry-first resolution
 Free Domain Browser is a CEF-based desktop browser for the Free Domain Name
 registry. It resolves registry names before falling back to public DNS.
CONTROL
if command -v dpkg-deb >/dev/null 2>&1; then
  DEB="target/$NAME.deb"
  rm -f "$DEB"
  dpkg-deb --build "$DEBROOT" "$DEB" >/dev/null
  echo "wrote $DEB"
else
  echo "dpkg-deb not found; Debian tree left at $DEBROOT" >&2
fi
