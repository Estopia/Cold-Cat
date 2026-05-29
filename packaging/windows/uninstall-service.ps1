$ErrorActionPreference = "Stop"

$serviceName = "ColdCatDaemon"

if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
    Stop-Service -Name $serviceName -ErrorAction SilentlyContinue
    sc.exe delete $serviceName | Out-Null
    Write-Host "Removed $serviceName"
} else {
    Write-Host "$serviceName is not installed"
}

