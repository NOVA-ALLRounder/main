param(
  [ValidateSet("strict", "balanced", "permissive")]
  [string]$Level = "strict",
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$scriptPath = Join-Path $RepoPath "scripts\\set_privacy_level.py"
if (-not (Test-Path $scriptPath)) {
  Write-Host "??set_privacy_level.py not found: $scriptPath"
  exit 1
}

$conda = Get-Command conda -ErrorAction SilentlyContinue
if ($conda) {
  $condaExe = if ($conda.Path) { $conda.Path } else { "conda" }
  & $condaExe run -n DATA_C python $scriptPath --level $Level
} else {
  python $scriptPath --level $Level
}
