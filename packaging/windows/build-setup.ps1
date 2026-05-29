$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Push-Location $repoRoot
try {
    cargo build --release -p coldcat-cli -p coldcat-daemon -p coldcat-gui

    $payloadDir = Join-Path $repoRoot "target\release"
    $env:COLD_CAT_PAYLOAD_DIR = $payloadDir
    cargo build --release -p coldcat-setup
    Remove-Item Env:COLD_CAT_PAYLOAD_DIR -ErrorAction SilentlyContinue

    $dist = Join-Path $repoRoot "dist"
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    Copy-Item (Join-Path $repoRoot "target\release\coldcat-setup.exe") (Join-Path $dist "ColdCatSetup.exe") -Force
    Write-Host "Built bundled installer: $(Join-Path $dist 'ColdCatSetup.exe')"
}
finally {
    Pop-Location
}

