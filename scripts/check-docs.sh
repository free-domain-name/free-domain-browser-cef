#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 Free Domain Name contributors

# Checks the pages under docs/ without building the browser. The Rust CI is
# skipped for pushes that only touch these pages, so this is what keeps a
# broken page from reaching the site.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

command -v python3 >/dev/null 2>&1 || {
	echo "check-docs: python3 is required" >&2
	exit 1
}

python3 - <<'PY'
import pathlib
import re
import sys

docs = pathlib.Path("docs")
errors = []

# Every page has to exist and carry the license header.
pages = sorted(docs.glob("*.html"))
if not pages:
	errors.append("docs/ has no HTML pages")

for page in pages:
	text = page.read_text(encoding="utf-8")

	if "SPDX-License-Identifier" not in text:
		errors.append(f"{page}: no SPDX license header")

	# Every local file the page points at has to be there.
	for ref in re.findall(r'(?:href|src)="([^"#:]+)"', text):
		if ref.startswith(("http://", "https://", "//", "mailto:", "data:")):
			continue
		target = (page.parent / ref).resolve()
		if not target.exists():
			errors.append(f"{page}: points at a missing file: {ref}")

	# Every copy button has to point at an element that is on the page.
	for selector in re.findall(r'data-copy="#([^"]+)"', text):
		if f'id="{selector}"' not in text:
			errors.append(f"{page}: copy button points at a missing id: #{selector}")

	# The page must not carry a script from a third party.
	for src in re.findall(r'<script[^>]+src="([^"]+)"', text):
		if not src.startswith(("site.js", "/")):
			errors.append(f"{page}: loads a script from elsewhere: {src}")

	# Every id has to be unique, or the anchors break.
	ids = re.findall(r'id="([^"]+)"', text)
	dupes = {i for i in ids if ids.count(i) > 1}
	for dupe in sorted(dupes):
		errors.append(f"{page}: duplicate id: {dupe}")

	# A page with an install section has to carry the commands that point at
	# the scripts the site serves, or a copy button would hand out something
	# that does not work.
	if 'class="install-list"' in text:
		for command in (
			"curl -fsSL https://free-domain-name.github.io/install.sh | sh",
			"irm https://free-domain-name.github.io/install.ps1 | iex",
		):
			if command not in text:
				errors.append(f"{page}: does not carry the install command: {command}")

if errors:
	for error in errors:
		print(f"check-docs: {error}", file=sys.stderr)
	sys.exit(1)

print(f"check-docs: {len(pages)} pages checked, no problems found")
PY
