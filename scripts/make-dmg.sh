#!/bin/sh
# Build the release disk image from the app bundle.
#
# hdiutil occasionally reports "Resource busy" or "Directory not empty" when the
# volume name is still held by the system or when the destination file was
# just removed. The stale mount is cleared and the build is retried.
set -eu
cd "$(dirname "$0")/.."

APP="target/bundle/free-domain-browser-cef.app"
DMG="target/FreeDomainBrowser-CEF.dmg"
VOLUME="Free Domain Browser"

if [ ! -d "$APP" ]; then
  echo "missing $APP; run scripts/package-release.sh first" >&2
  exit 1
fi

hdiutil detach "/Volumes/$VOLUME" -quiet -force >/dev/null 2>&1 || true

attempt=1
while [ "$attempt" -le 3 ]; do
  rm -f "$DMG"
  if hdiutil create -volname "$VOLUME" -srcfolder "$APP" -ov -format UDZO "$DMG"; then
    echo "wrote $DMG"
    exit 0
  fi
  echo "hdiutil failed (attempt $attempt)" >&2
  attempt=$((attempt + 1))
  sleep 5
done

echo "DMG creation failed" >&2
exit 1
