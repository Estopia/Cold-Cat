$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$daemon = Join-Path $repoRoot "target\release\coldcat-daemon.exe"

if (-not (Test-Path $daemon)) {
    throw "Missing $daemon. Run 'cargo build --release' first."
}

$serviceName = "ColdCatDaemon"
$binaryPath = "`"$daemon`" service"

if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
    Stop-Service -Name $serviceName -ErrorAction SilentlyContinue
    sc.exe delete $serviceName | Out-Null
    Start-Sleep -Seconds 2
}

New-Service `
    -Name $serviceName `
    -BinaryPathName $binaryPath `
    -DisplayName "Cold Cat Daemon" `
    -Description "Privileged Cold Cat website and app blocking service." `
    -StartupType Automatic | Out-Null

sc.exe failure $serviceName reset= 0 actions= restart/1000/restart/1000/restart/1000 | Out-Null
Start-Service -Name $serviceName
Write-Host "Installed and started $serviceName"

