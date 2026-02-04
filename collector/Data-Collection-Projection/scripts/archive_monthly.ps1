param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ArchiveDir = "archive\\raw",
  [string]$MonthlyDir = "archive\\monthly",
  [switch]$DeleteAfter,
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

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

$argsList = @("scripts\\compact_archive_monthly.py", "--archive-dir", $ArchiveDir, "--output-dir", $MonthlyDir)
if ($DeleteAfter) { $argsList += "--delete-after" }
Invoke-Python $argsList

Invoke-Python @("scripts\\archive_manifest.py", "--archive-dir", $ArchiveDir, "--include-monthly", "--monthly-dir", $MonthlyDir, "--output", (Join-Path $ArchiveDir "..\\manifest.json"))
