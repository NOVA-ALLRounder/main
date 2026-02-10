param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [switch]$NoActivate
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$postScript = Join-Path $RepoPath "collector\\Data-Collection-Projection\\scripts\\run_post_collection.ps1"
$bootstrap = Join-Path $RepoPath "scripts\\n8n_bootstrap.ps1"

if (-not (Test-Path $postScript)) {
  Write-Host "❌ post_collection script not found: $postScript"
  exit 1
}
if (-not (Test-Path $bootstrap)) {
  Write-Host "❌ n8n bootstrap script not found: $bootstrap"
  exit 1
}

Write-Host "▶ Running post-collection pipeline..."
& $postScript -ConfigPath $ConfigPath

Write-Host "▶ Importing workflow into n8n..."
if ($NoActivate) {
  & $bootstrap -NoActivate
} else {
  & $bootstrap
}
