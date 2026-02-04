param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$CondaEnv = "DATA_C",
  [string]$ConfigPath = "configs\\config.yaml",
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

$pythonArgs = @("-m", "collector.main", "--config", $resolvedConfig)

if (-not $NoConda) {
  $conda = Get-Command conda -ErrorAction SilentlyContinue
  if ($conda) {
    & $conda.Path run -n $CondaEnv python @pythonArgs
    exit $LASTEXITCODE
  }
}

& python @pythonArgs
exit $LASTEXITCODE
