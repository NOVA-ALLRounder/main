$ErrorActionPreference = "Stop"

Write-Host "🚀 [Steer Agent] Starting Release Build Process..."
Write-Host "---------------------------------------------------"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Resolve-Path (Join-Path $scriptDir "..")
$appsRoot = Join-Path $projectRoot "apps"

# 1. Build Frontend
Write-Host "📦 [1/3] Building Web Frontend..."
Push-Location (Join-Path $appsRoot "web")

npm install
npm run build

Pop-Location

# 2. Build Tauri Desktop App
Write-Host "🦀 [2/3] Building Desktop App (Rust + Tauri)..."
Push-Location (Join-Path $appsRoot "desktop")

if (-not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
    Write-Host "ℹ️  Installing tauri-cli..."
    cargo install tauri-cli --version "^2.0.0" --locked
}

cargo tauri build

Pop-Location

# 3. Locate Artifacts
Write-Host "🎉 [3/3] Build Complete!"
Write-Host "---------------------------------------------------"
Write-Host "✅ Release Bundle Location:"

$bundlePath = Join-Path $appsRoot "desktop\src-tauri\target\release\bundle"
if (Test-Path $bundlePath) {
    Get-ChildItem -Path $bundlePath | ForEach-Object { $_.FullName }
} else {
    Write-Host "   (Check apps\desktop\src-tauri\target\release\bundle)"
}

Write-Host "---------------------------------------------------"
Write-Host "👉 You can now distribute the app from the folder above."
