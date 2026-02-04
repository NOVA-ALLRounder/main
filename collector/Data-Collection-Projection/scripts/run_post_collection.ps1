param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config_run4.yaml",
  [string]$OutputDir = "logs\\run4",
  [int]$MaxBytes = 8000,
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}

if (-not $NoConda) {
  $conda = Get-Command conda -ErrorAction SilentlyContinue
  if ($conda) {
    & $conda.Path run -n DATA_C python scripts\build_daily_summary.py --config $resolvedConfig --store-db
    & $conda.Path run -n DATA_C python scripts\build_pattern_summary.py --summaries-dir $OutputDir --since-days 7 --config $resolvedConfig --store-db
    $daily = Get-ChildItem $OutputDir -Filter "daily_summary_*.json" | Sort-Object LastWriteTime | Select-Object -Last 1
    if ($daily) {
      & $conda.Path run -n DATA_C python scripts\build_llm_input.py --config $resolvedConfig --daily $daily.FullName --pattern (Join-Path $OutputDir "pattern_summary.json") --output (Join-Path $OutputDir "llm_input.json") --max-bytes $MaxBytes --store-db
      & $conda.Path run -n DATA_C python scripts\generate_recommendations.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output-md (Join-Path $OutputDir "activity_recommendations.md") --output-json (Join-Path $OutputDir "activity_recommendations.json")
    }
    exit $LASTEXITCODE
  }
}

python scripts\build_daily_summary.py --config $resolvedConfig --store-db
python scripts\build_pattern_summary.py --summaries-dir $OutputDir --since-days 7 --config $resolvedConfig --store-db
$dailyLocal = Get-ChildItem $OutputDir -Filter "daily_summary_*.json" | Sort-Object LastWriteTime | Select-Object -Last 1
if ($dailyLocal) {
  python scripts\build_llm_input.py --config $resolvedConfig --daily $dailyLocal.FullName --pattern (Join-Path $OutputDir "pattern_summary.json") --output (Join-Path $OutputDir "llm_input.json") --max-bytes $MaxBytes --store-db
  python scripts\generate_recommendations.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output-md (Join-Path $OutputDir "activity_recommendations.md") --output-json (Join-Path $OutputDir "activity_recommendations.json")
}
