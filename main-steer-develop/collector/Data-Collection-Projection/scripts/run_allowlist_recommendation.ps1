param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$CondaEnv = "DATA_C",
  [string]$ConfigPath = "configs\\config.yaml",
  [double]$Days = 3,
  [double]$MinMinutes = 10,
  [int]$MinBlocks = 3,
  [switch]$Apply,
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}
if (-not (Test-Path $resolvedConfig)) {
  throw "Config not found: $resolvedConfig"
}

$pythonArgs = @(
  "scripts\\recommend_allowlist.py",
  "--config", $resolvedConfig,
  "--days", $Days,
  "--min-minutes", $MinMinutes,
  "--min-blocks", $MinBlocks
)

if ($Apply) {
  $pythonArgs += "--apply"
}

if (-not $NoConda) {
  $conda = Get-Command conda -ErrorAction SilentlyContinue
  if ($conda) {
    & $conda.Path run -n $CondaEnv python @pythonArgs
    exit $LASTEXITCODE
  }
}

& python @pythonArgs
exit $LASTEXITCODE
