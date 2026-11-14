#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
ENGINE=${ENGINE:-docker}
"$ENGINE" build -t free-domain-browser-cef-core-test -f Dockerfile.core .
"$ENGINE" run --rm free-domain-browser-cef-core-test
