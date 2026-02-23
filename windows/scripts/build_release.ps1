param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [switch]$Install
)

$ErrorActionPreference = "Stop"

$runWeb = Join-Path $PSScriptRoot "run_web.ps1"
& $runWeb -RepoPath $RepoPath -Mode build -Install:$Install
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

$tauriDir = Join-Path $RepoPath "desktop\\src-tauri"
Set-Location $tauriDir

& cargo tauri build
exit $LASTEXITCODE
