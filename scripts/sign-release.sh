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
  codesign --force --timestamp --options runtime \
    --entitlements "$ENTITLEMENTS" --sign "$IDENTITY" "$1"
}

# Sign helper executables first, then the framework, then the outer app.
find "$APP/Contents/Frameworks" -type f -path '*/Contents/MacOS/*' -print0 |
  while IFS= read -r -d '' helper; do
    sign "$helper"
  done

if [ -d "$APP/Contents/Frameworks/Chromium Embedded Framework.framework" ]; then
  sign "$APP/Contents/Frameworks/Chromium Embedded Framework.framework"
fi

sign "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
echo "signed $APP"
