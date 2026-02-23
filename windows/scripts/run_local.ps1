param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [ValidateSet("rust","python")] [string]$CollectorImpl = "rust",
  [string]$CondaEnv = "DATA_C",
  [string]$ConfigPath = "configs\\config.yaml",
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

$runCore = Join-Path $PSScriptRoot "run_core.ps1"
if (-not (Test-Path $runCore)) {
  throw "run_core.ps1 not found at $runCore"
}

& $runCore `
  -RepoPath $RepoPath `
  -CollectorImpl $CollectorImpl `
  -CondaEnv $CondaEnv `
  -ConfigPath $ConfigPath `
  -NoConda:$NoConda

exit $LASTEXITCODE
