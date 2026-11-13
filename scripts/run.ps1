# Windows launcher: CEF is downloaded by the first cargo build into .\cef-binary.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not $env:CEF_PATH) {
    $env:CEF_PATH = Join-Path $root "cef-binary"
}
cargo run -- @args
exit $LASTEXITCODE
