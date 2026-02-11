$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$coreDir = Resolve-Path (Join-Path $scriptDir "..\apps\core")
$guardianScript = Resolve-Path (Join-Path $scriptDir "steer-guardian.ps1")

Write-Host "🚀 Installing Local OS Agent as a Startup Task (Windows)..."

# 1. Build Release Binary
Write-Host "📦 Building core binary..."
Push-Location $coreDir
cargo build --release
Pop-Location

# 2. Create Startup shortcut
$startup = [Environment]::GetFolderPath("Startup")
$shortcutPath = Join-Path $startup "Steer Guardian.lnk"

$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($shortcutPath)
$shortcut.TargetPath = "powershell.exe"
$shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$guardianScript`""
$shortcut.WorkingDirectory = $scriptDir
$shortcut.Save()

Write-Host "✅ Success! The Agent will now start automatically on login."
Write-Host "   - Startup shortcut: $shortcutPath"
Write-Host "   - Guardian script: $guardianScript"
Write-Host "   - Web Dashboard: http://localhost:5680"
