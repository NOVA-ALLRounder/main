param(
    [string]$CondaEnv = "DATA_C",
    [string]$ConfigPath = "configs\config.yaml"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$dcpRoot = Join-Path $repoRoot "collector\\Data-Collection-Projection"
$runner = Join-Path $dcpRoot "scripts\run_core.ps1"

if (-not (Test-Path $runner)) {
    Write-Host "❌ Data-Collection-Projection runner not found: $runner"
    exit 1
}

$fullConfig = if ([System.IO.Path]::IsPathRooted($ConfigPath)) {
    $ConfigPath
} else {
    Join-Path $dcpRoot $ConfigPath
}

Write-Host "▶ Starting Data-Collection-Projection..."
& $runner -CondaEnv $CondaEnv -ConfigPath $fullConfig
