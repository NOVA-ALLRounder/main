param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config_run4.yaml",
  [string]$ArchiveDir = "archive\\raw",
  [int]$Days = 1,
  [switch]$DeleteAfter,
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}

function Invoke-Python {
  param([string[]]$Args)
  if (-not $NoConda) {
    $conda = Get-Command conda -ErrorAction SilentlyContinue
    if ($conda) {
      & $conda.Path run -n DATA_C python @Args
      return
    }
  }
  & python @Args
}

$argsList = @("scripts\\archive_raw_events.py", "--config", $resolvedConfig, "--days", $Days, "--output-dir", $ArchiveDir)
if ($DeleteAfter) { $argsList += "--delete-after" }
Invoke-Python $argsList

Invoke-Python @("scripts\\archive_manifest.py", "--archive-dir", $ArchiveDir, "--output", (Join-Path $ArchiveDir "..\\manifest.json"))
