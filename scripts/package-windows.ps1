# Build a portable Windows x64 package from the CEF bootstrap output.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
if ($env:FDB_SKIP_BUILD -ne "1") {
  cargo run --release --bin bundle -- --build-only --release
}
$name = "FreeDomainBrowser-CEF-windows-x64"
$root = "target/package"
$stage = Join-Path $root $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item -Recurse -Force "target/bootstrap/release/*" $stage
Copy-Item -Force "icons/icon.ico" (Join-Path $stage "free-domain-browser-cef.ico")
Copy-Item -Force "LICENSE" (Join-Path $stage "LICENSE")
$out = "target/$name.zip"
if (Test-Path $out) { Remove-Item -Force $out }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $out
Write-Host "wrote $out"

# Build an NSIS installer when makensis is available; the portable zip remains
# the fallback on machines without NSIS.
$nsis = Get-Command makensis -ErrorAction SilentlyContinue
if ($null -ne $nsis) {
  $stageAbs = (Resolve-Path $stage).Path
  $setup = Join-Path (Get-Location) "target/$name-setup.exe"
  Remove-Item -Force $setup -ErrorAction SilentlyContinue
  & $nsis.Source "/DSTAGE=$stageAbs" "/DOUTFILE=$setup" "assets/windows/installer.nsi"
  if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
  Write-Host "wrote $setup"
} else {
  Write-Host "makensis not found; skipping Windows installer"
}
