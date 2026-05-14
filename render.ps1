#!/usr/bin/env pwsh
param([switch]$open)

Set-Location $PSScriptRoot\gospel-render
cargo run --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($open) {
    Start-Process "$PSScriptRoot\site\index.html"
}
