param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

$hostName = if ($env:STEER_BG_PANEL_HOST) { $env:STEER_BG_PANEL_HOST } else { "127.0.0.1" }
$port = if ($env:STEER_BG_PANEL_PORT) { $env:STEER_BG_PANEL_PORT } else { "8787" }

Write-Host "Starting Steer BG panel at http://$hostName`:$port"

$python = Get-Command python -ErrorAction SilentlyContinue
if (-not $python) {
  $python = Get-Command python3 -ErrorAction SilentlyContinue
}
if (-not $python) {
  throw "python/python3 not found in PATH"
}

& $python.Source "scripts/run_nl_bg_panel.py"
exit $LASTEXITCODE
