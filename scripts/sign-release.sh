#!/bin/bash
# Sign the release .app after scripts/package-release.sh. Requires a Developer ID
# Application identity installed in the login keychain.
set -eu
cd "$(dirname "$0")/.."

IDENTITY="${1:-${CEF_CODESIGN_IDENTITY:-}}"
if [ -z "$IDENTITY" ]; then
  echo "usage: scripts/sign-release.sh 'Developer ID Application: ...'" >&2
  exit 1
fi

APP="target/bundle/free-domain-browser-cef.app"
ENTITLEMENTS="assets/entitlements.mac.plist"

if [ ! -d "$APP" ]; then
  echo "missing $APP; run scripts/package-release.sh first" >&2
  exit 1
fi

sign() {
  # The hardened runtime is required: Apple's notary rejects an archive whose
  # executables do not have it ("The executable does not have the hardened
  # runtime enabled").
  codesign --force --timestamp --options runtime \
    --entitlements "$ENTITLEMENTS" --sign "$IDENTITY" "$1"
}

# The libraries inside the framework are nested code of their own. They come
# with a linker signature only, and the notary rejects the whole archive when
# one of them is not signed with a real certificate.
sign_library() {
  codesign --force --timestamp --sign "$IDENTITY" "$1"
}

# Sign the nested libraries first, then the helper executables, then the
# framework, then the outer app: a container is signed after its contents.
find "$APP/Contents/Frameworks" \( -name "*.dylib" -o -name "*.so" \) -type f -print0 |
  while IFS= read -r -d '' library; do
    sign_library "$library"
  done

find "$APP/Contents/Frameworks" -type f -path '*/Contents/MacOS/*' -print0 |
  while IFS= read -r -d '' helper; do
    sign "$helper"
  done

if [ -d "$APP/Contents/Frameworks/Chromium Embedded Framework.framework" ]; then
  sign "$APP/Contents/Frameworks/Chromium Embedded Framework.framework"
fi

sign "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# The package script writes an ad-hoc-signed zip before this step. Replace it
# with the signed bundle so the downloadable artifact matches the signature.
OUT="target/FreeDomainBrowser-CEF.zip"
rm -f "$OUT"
ditto -c -k --keepParent "$APP" "$OUT"
echo "signed $APP"
echo "wrote $OUT"
