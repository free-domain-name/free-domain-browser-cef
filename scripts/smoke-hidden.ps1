# Headless smoke test on Windows: start the packaged browser, wait for the
# DevTools endpoint, list the targets and stop the browser again.
#
# The package has to carry the CEF runtime next to the executable; the check
# below fails when it does not.
#
# The DevTools endpoint is read with curl.exe, the same way the shell script
# reads it on macOS and Linux.
#
# The browser is started twice when the endpoint stays quiet: once with the
# native menu bar and once without it. That tells the two apart.
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

$port = if ($env:FDB_SMOKE_PORT) { $env:FDB_SMOKE_PORT } else { "9222" }
$url = if ($env:FDB_SMOKE_URL) { $env:FDB_SMOKE_URL } else { "https://github.com/" }
$expect = $env:FDB_SMOKE_EXPECT
$stage = if ($env:FDB_STAGE) { $env:FDB_STAGE } else { "target/package/FreeDomainBrowser-CEF-windows-x64" }
$exe = Join-Path $stage "free-domain-browser-cef.exe"

if (-not (Test-Path $exe)) { throw "the packaged browser is missing: $exe" }
if (-not (Test-Path (Join-Path $stage "libcef.dll"))) {
  throw "the package does not carry libcef.dll next to the executable"
}

$logDir = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { $env:TEMP }
$out = Join-Path $logDir "free-domain-browser-cef-smoke.out.log"
$err = Join-Path $logDir "free-domain-browser-cef-smoke.err.log"

function Get-Cdp([string]$path) {
  $body = & curl.exe -s --fail --max-time 5 "http://127.0.0.1:$port$path" 2>$null
  if ($LASTEXITCODE -ne 0) { return $null }
  if (-not $body) { return $null }
  return ($body -join "`n")
}

function Show-Log {
  Write-Host "browser log:"
  Get-Content $out -ErrorAction SilentlyContinue | Select-Object -Last 25
  Get-Content $err -ErrorAction SilentlyContinue | Select-Object -Last 25
}

function Show-Probe {
  Write-Host "DevTools probe:"
  & curl.exe -v --max-time 5 "http://127.0.0.1:$port/json/version" 2>&1 | Select-Object -Last 15
}

function Stop-Browser($proc) {
  if ($null -ne $proc) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  }
  Get-Process -Name "free-domain-browser-cef", "free_domain_browser_cef_helper" `
    -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

function Start-Browser {
  Remove-Item -Force $out, $err -ErrorAction SilentlyContinue
  return Start-Process -FilePath $exe `
    -ArgumentList @("--remote-debugging-port=$port", "--url=$url", "--no-sandbox", "--enable-logging=stderr") `
    -WorkingDirectory (Resolve-Path $stage).Path `
    -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
}

# First try: the browser as it is shipped, with the native menu bar.
$env:FDB_WINDOWS_MENU = "post"
$proc = Start-Browser
$ready = $false
for ($i = 0; $i -lt 60; $i++) {
  if (Get-Cdp "/json/version") { $ready = $true; break }
  if ($proc.HasExited) { break }
  Start-Sleep -Seconds 1
}

if (-not $ready) {
  Write-Host "the DevTools endpoint stayed quiet with the native menu bar"
  Show-Log
  Stop-Browser $proc

  # Second try: without the menu bar, to tell the menu apart from the rest.
  $env:FDB_WINDOWS_MENU = "none"
  $proc = Start-Browser
  for ($i = 0; $i -lt 60; $i++) {
    if (Get-Cdp "/json/version") { $ready = $true; break }
    if ($proc.HasExited) { break }
    Start-Sleep -Seconds 1
  }
  if ($ready) {
    Write-Host "the DevTools endpoint only answers without the native menu bar"
    Show-Log
    Stop-Browser $proc
    exit 1
  }
  Write-Host "the DevTools endpoint never answered"
  Show-Log
  Show-Probe
  Stop-Browser $proc
  exit 1
}

Write-Host "CEF DevTools ready on $port"
$targets = (Get-Cdp "/json") | ConvertFrom-Json
Write-Host "$($targets.Count) targets"
foreach ($target in $targets) { Write-Host $target.url }

if ($expect) {
  $found = $targets | Where-Object { $_.url -like "*$expect*" }
  if (-not $found) {
    Write-Host "expected target not found: $expect"
    Show-Log
    Stop-Browser $proc
    exit 1
  }
}
if ($proc.HasExited) {
  Write-Host "the browser exited early:"
  Show-Log
  exit 1
}
Write-Host "the packaged browser is running"
Stop-Browser $proc
exit 0
