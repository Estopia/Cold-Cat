$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$gui = Join-Path $repoRoot "target\release\coldcat-gui.exe"

if (-not (Test-Path $gui)) {
    throw "Missing $gui. Run 'cargo build --release' first."
}

$shell = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath("Desktop")
$shortcutPath = Join-Path $desktop "Cold Cat.lnk"
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $gui
$shortcut.WorkingDirectory = Split-Path $gui
$shortcut.Description = "Launch the Cold Cat blocker GUI"
$shortcut.Save()

Write-Host "Created desktop shortcut: $shortcutPath"

