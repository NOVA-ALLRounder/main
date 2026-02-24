param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path
)

$ErrorActionPreference = "Stop"

$installService = Join-Path $PSScriptRoot "install_service.ps1"
if (-not (Test-Path $installService)) {
  throw "install_service.ps1 not found: $installService"
}

& $installService
exit $LASTEXITCODE
