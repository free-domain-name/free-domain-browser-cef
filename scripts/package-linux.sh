#!/bin/sh
# Build a portable Linux x64 package from the CEF bootstrap output.
# The package is a tar.gz containing the browser, helper, CEF runtime and a
# desktop entry. It does not need root to run.
#
# FDB_PROFILE=debug|release picks the cargo profile (default: release).
# FDB_SKIP_BUILD=1 reuses an existing build.
# FDB_BIN_DIR overrides the directory the binaries are taken from.
set -eu
cd "$(dirname "$0")/.."
PROFILE="${FDB_PROFILE:-release}"
if [ "${FDB_SKIP_BUILD:-0}" != "1" ]; then
  if [ "$PROFILE" = "release" ]; then
    cargo run --release --bin bundle -- --build-only --release
  else
    cargo run --bin bundle -- --build-only
  fi
fi
ARCH="$(uname -m)"
NAME="FreeDomainBrowser-CEF-linux-${ARCH}"
ROOT="target/package"
STAGE="$ROOT/$NAME"
BIN_DIR="${FDB_BIN_DIR:-target/bootstrap/$PROFILE}"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -R "$BIN_DIR"/. "$STAGE/"
rm -f "$STAGE"/*.d

# The CEF runtime has to travel with the browser. The loader looks for
# libcef.so next to the executable and CEF reads its locales and resources
# from the same directory. Without them the package does not start at all.
CEF_ROOT="${CEF_PATH:-cef-binary}"
runtime=""
for candidate in "$CEF_ROOT" "$CEF_ROOT"/* "$CEF_ROOT"/*/*; do
  if [ -f "$candidate/libcef.so" ]; then
    runtime="$candidate"
    break
  fi
done
if [ -z "$runtime" ]; then
  echo "libcef.so not found below $CEF_ROOT" >&2
  exit 1
fi
for entry in "$runtime"/*; do
  [ -e "$entry" ] || continue
  base="$(basename "$entry")"
  case "$base" in
    # Development files are not needed to run the browser.
    include|cmake|CMakeLists.txt|*.tar.bz2) continue ;;
  esac
  cp -R "$entry" "$STAGE/"
done
echo "CEF runtime: $runtime"

# The launcher keeps the loader and CEF on the runtime directory and deals
# with the setuid sandbox, which a tarball cannot ship as root.
mv "$STAGE/free-domain-browser-cef" "$STAGE/free-domain-browser-cef.bin"
cat > "$STAGE/free-domain-browser-cef" <<'LAUNCHER'
#!/bin/sh
# Free Domain Browser launcher.
#
# It points the loader and CEF at the bundled runtime and falls back to
# --no-sandbox when the setuid sandbox helper is not installed, which is the
# case for an unprivileged tarball. Set FDB_SANDBOX=1 to keep the sandbox
# anyway, or FDB_SANDBOX=0 to always run without it.
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CEF_PATH="$here"
LD_LIBRARY_PATH="$here${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export CEF_PATH LD_LIBRARY_PATH

sandbox_ready() {
  [ -e "$here/chrome-sandbox" ] || return 1
  perms=$(stat -c '%u %a' "$here/chrome-sandbox" 2>/dev/null) || return 1
  set -- $perms
  [ "$1" = "0" ] || return 1
  case "$2" in
    4*|5*|6*) return 0 ;;
    *) return 1 ;;
  esac
}

case "${FDB_SANDBOX:-auto}" in
  1) extra="" ;;
  0) extra="--no-sandbox" ;;
  *)
    if sandbox_ready; then
      extra=""
    else
      extra="--no-sandbox"
      echo "The setuid sandbox helper is not installed, so the browser runs" >&2
      echo "without it. To enable it, run:" >&2
      echo "  sudo chown root:root '$here/chrome-sandbox' && sudo chmod 4755 '$here/chrome-sandbox'" >&2
    fi
    ;;
esac
exec "$here/free-domain-browser-cef.bin" "$@" $extra
LAUNCHER
chmod 755 "$STAGE/free-domain-browser-cef"

cat > "$STAGE/README.txt" <<'PACKAGE_README'
Free Domain Browser (CEF build) - portable Linux package
=========================================================

Run the browser:

    ./free-domain-browser-cef

It opens the bundled Chromium runtime from this directory, so the package
is self-contained. The setuid sandbox helper cannot be shipped as root
inside a tarball; the launcher says so on the first start and runs without
the sandbox until you install it:

    sudo chown root:root chrome-sandbox
    sudo chmod 4755 chrome-sandbox

Install the .deb package instead for the full sandbox and a desktop entry.
PACKAGE_README

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
chmod 755 "$STAGE/free-domain-browser-cef.bin" "$STAGE/free_domain_browser_cef_helper" 2>/dev/null || true
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
# The sandbox helper has to be owned by root and setuid, which dpkg does when
# it installs the package.
if [ -f "$DEBROOT/usr/lib/$PKG/chrome-sandbox" ]; then
  chmod 4755 "$DEBROOT/usr/lib/$PKG/chrome-sandbox"
fi
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
Depends: libgtk-3-0, libnss3, libgbm1, libasound2, libxss1, libxtst6, libatk-bridge2.0-0, libcups2, libdrm2, libxkbcommon0, libpango-1.0-0, libcairo2
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
