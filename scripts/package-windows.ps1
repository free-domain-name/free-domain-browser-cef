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
  # PowerShell does not fail on a native command that exits with an error, so
  # the exit code has to be checked by hand. Without it a broken build was
  # packaged as if it had worked.
  if ($LASTEXITCODE -ne 0) { throw "the browser build failed with exit code $LASTEXITCODE" }
}
$name = "FreeDomainBrowser-CEF-windows-x64"
$root = "target/package"
$stage = Join-Path $root $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path $stage | Out-Null
$binDir = if ($env:FDB_BIN_DIR) { $env:FDB_BIN_DIR } else { "target/bootstrap/$profile" }
# Only the two binaries belong in the package. Copying the whole build
# directory would put the dependency tree into the archive.
foreach ($bin in @("free-domain-browser-cef.exe", "free_domain_browser_cef_helper.exe")) {
  $source = Join-Path $binDir $bin
  if (-not (Test-Path $source)) { throw "missing $source" }
  Copy-Item -Force $source $stage
}
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
Get-ChildItem $runtime | Where-Object { $_.Name -notin @("include", "cmake", "CMakeLists.txt", "libcef_dll") } |
  ForEach-Object { Copy-Item -Recurse -Force $_.FullName $stage }
Write-Host "CEF runtime: $runtime"

$out = "target/$name.zip"
if (Test-Path $out) { Remove-Item -Force $out }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $out
Write-Host "wrote $out"

# Build an NSIS installer when makensis is available; the portable zip remains
# the fallback on machines without NSIS.
#
# A machine that just installed NSIS does not have it on PATH yet: the
# runner picks up the new PATH value with the next process, so the usual
# install directories are searched as well.
$nsisPath = $null
$command = Get-Command makensis -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -ne $command) {
  $nsisPath = $command.Source
} else {
  $known = @(
    "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
    "$env:ProgramFiles\NSIS\makensis.exe",
    "$env:LOCALAPPDATA\Programs\NSIS\makensis.exe"
  )
  foreach ($path in $known) {
    if ($path -and (Test-Path $path)) {
      $nsisPath = $path
      break
    }
  }
}
if ($nsisPath) {
  $stageAbs = (Resolve-Path $stage).Path
  $setup = Join-Path (Get-Location) "target/$name-setup.exe"
  Remove-Item -Force $setup -ErrorAction SilentlyContinue
  # The version comes from the manifest, so the installer never says a
  # version of its own.
  $version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches.Groups[1].Value
  if (-not $version) { throw "could not read the version from Cargo.toml" }
  & $nsisPath "/DSTAGE=$stageAbs" "/DOUTFILE=$setup" "/DVERSION=$version" "assets/windows/installer.nsi"
  if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
  Write-Host "wrote $setup"
} else {
  Write-Host "makensis not found; skipping Windows installer"
}
