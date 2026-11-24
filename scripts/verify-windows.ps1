# Run the verification journeys against the Windows build.
#
#   pwsh ./scripts/verify-windows.ps1
#   pwsh ./scripts/verify-windows.ps1 first-launch
#
# The journeys drive the real browser through the DevTools protocol, so this
# is the same check the macOS and Linux runners do, on the Windows build.
# On Windows the journeys run against the packaged build, because that is
# the one that carries the CEF runtime next to the executable.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

# The runner speaks the DevTools protocol over a WebSocket. Node ships a
# WebSocket client from version 20.10 on, and an older Node has none.
$nodeMajor = 0
try {
  $nodeMajor = [int]((node --version).TrimStart("v").Split(".")[0])
} catch {
  throw "node is needed to run the verification journeys"
}
if ($nodeMajor -lt 20) {
  throw "node 20 or newer is needed to run the verification journeys, found $nodeMajor"
}

$stage = if ($env:FDB_STAGE) { $env:FDB_STAGE } else { "target/package/FreeDomainBrowser-CEF-windows-x64" }
$exe = Join-Path $stage "free-domain-browser-cef.exe"
if (-not (Test-Path $exe)) {
  throw "the packaged browser is missing: $exe (run scripts/package-windows.ps1 first)"
}

# The journeys start the browser themselves, so the runner has to know which
# build to start.
$env:FDB_CEF_BINARY = (Resolve-Path $exe).Path

node verification/run.mjs @args
exit $LASTEXITCODE
