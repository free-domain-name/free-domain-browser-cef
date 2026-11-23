#!/bin/bash
# Run the verification journeys against a real build of the browser.
#
#   scripts/verify.sh                every journey
#   scripts/verify.sh first-launch   one journey
#
# The build is made if it is missing. On a machine without a screen, set
# FDB_VERIFY_HIDDEN=1 and the browser starts hidden.
set -eu
cd "$(dirname "$0")/.."
if [ ! -x target/debug/free-domain-browser-cef ] && [ ! -d target/bundle ]; then
  scripts/build.sh
fi
exec node verification/run.mjs "$@"
