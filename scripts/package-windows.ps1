# Build a portable Windows x64 package from the CEF bootstrap output.
#
# FDB_PROFILE=debug|release picks the cargo profile (default: release).
# FDB_SKIP_BUILD=1 reuses an existing build.
# FDB_BIN_DIR overrides the directory the binaries are taken from.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)
$profile = if ($env:FDB_PROFILE) { $env:FDB_PROFILE } else { "release" }
if ($env:FDB_SKIP_BUILD -ne "1") {
  if ($profile -eq "release") {
    cargo run --release --bin bundle -- --build-only --release
  } else {
    cargo run --bin bundle -- --build-only
  }
}
$name = "FreeDomainBrowser-CEF-windows-x64"
$root = "target/package"
$stage = Join-Path $root $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
$binDir = if ($env:FDB_BIN_DIR) { $env:FDB_BIN_DIR } else { "target/bootstrap/$profile" }
Copy-Item -Recurse -Force "$binDir/*" $stage
Copy-Item -Force "icons/icon.ico" (Join-Path $stage "free-domain-browser-cef.ico")
Copy-Item -Force "LICENSE" (Join-Path $stage "LICENSE")

# The CEF runtime has to travel with the browser: Windows loads libcef.dll
# from the directory of the executable and CEF reads its locales and
# resources from the same place. Without them the package does not start.
$cefRoot = if ($env:CEF_PATH) { $env:CEF_PATH } else { "cef-binary" }
$runtime = $null
foreach ($candidate in @($cefRoot, "$cefRoot/*", "$cefRoot/*/*")) {
  $dir = Get-Item $candidate -ErrorAction SilentlyContinue |
    Where-Object { $_.PSIsContainer -and (Test-Path (Join-Path $_.FullName "libcef.dll")) } |
    Select-Object -First 1
  if ($null -ne $dir) { $runtime = $dir.FullName; break }
}
if ($null -eq $runtime) { throw "libcef.dll not found below $cefRoot" }
Get-ChildItem $runtime | Where-Object { $_.Name -notin @("include", "cmake", "CMakeLists.txt") } |
  ForEach-Object { Copy-Item -Recurse -Force $_.FullName $stage }
Write-Host "CEF runtime: $runtime"

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
