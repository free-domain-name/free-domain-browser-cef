# Headless smoke test on Windows: start the packaged browser, wait for the
# DevTools endpoint, list the targets and stop the browser again.
#
# The package has to carry the CEF runtime next to the executable; the check
# below fails when it does not.
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
Remove-Item -Force $out, $err -ErrorAction SilentlyContinue

$proc = Start-Process -FilePath $exe `
  -ArgumentList @("--remote-debugging-port=$port", "--url=$url", "--no-sandbox") `
  -WorkingDirectory (Resolve-Path $stage).Path `
  -RedirectStandardOutput $out -RedirectStandardError $err -PassThru

function Stop-Browser {
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  Get-Process -Name "free-domain-browser-cef", "free_domain_browser_cef_helper" `
    -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

$ready = $false
for ($i = 0; $i -lt 180; $i++) {
  try {
    $version = Invoke-RestMethod -Uri "http://127.0.0.1:$port/json/version" -TimeoutSec 2
    $ready = $true
    break
  } catch {
    if ($proc.HasExited) {
      Write-Host "the browser exited early; last log:"
      Get-Content $err -ErrorAction SilentlyContinue | Select-Object -Last 20
      exit 1
    }
    Start-Sleep -Seconds 1
  }
}
if (-not $ready) {
  Write-Host "timed out waiting for CEF; last log:"
  Get-Content $err -ErrorAction SilentlyContinue | Select-Object -Last 20
  Stop-Browser
  exit 1
}

Write-Host "CEF DevTools ready on $port"
$targets = Invoke-RestMethod -Uri "http://127.0.0.1:$port/json" -TimeoutSec 5
Write-Host "$($targets.Count) targets"
foreach ($target in $targets) { Write-Host $target.url }

if ($expect) {
  $found = $targets | Where-Object { $_.url -like "*$expect*" }
  if (-not $found) {
    Write-Host "expected target not found: $expect"
    Stop-Browser
    exit 1
  }
}
if ($proc.HasExited) {
  Write-Host "the browser exited early; last log:"
  Get-Content $err -ErrorAction SilentlyContinue | Select-Object -Last 20
  exit 1
}
Write-Host "the packaged browser is running"
Stop-Browser
exit 0
