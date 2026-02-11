param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [string]$OutputDir = "logs",
  [int]$MaxBytes = 8000,
  [switch]$NoConda
)

$ErrorActionPreference = "Stop"

Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}

if (-not $env:OPENAI_API_KEY) {
  $envPath = Join-Path $RepoPath ".env"
  if (Test-Path $envPath) {
    $line = Get-Content $envPath | Where-Object { $_ -match '^OPENAI_API_KEY=' } | Select-Object -First 1
    if ($line) {
      $env:OPENAI_API_KEY = ($line -split '=',2)[1].Trim()
    }
  }
}

if (-not $NoConda) {
  $conda = Get-Command conda -ErrorAction SilentlyContinue
  if ($conda) {
    $condaExe = if ($conda.Path) { $conda.Path } else { "conda" }
    & $condaExe run -n DATA_C python scripts\build_daily_summary.py --config $resolvedConfig --store-db
    & $condaExe run -n DATA_C python scripts\build_pattern_summary.py --summaries-dir $OutputDir --since-days 7 --config $resolvedConfig --store-db
    & $condaExe run -n DATA_C python scripts\learn_profile.py --config $resolvedConfig --output configs\profile_learned.json --days 14
    $daily = Get-ChildItem $OutputDir -Filter "daily_summary_*.json" | Sort-Object LastWriteTime | Select-Object -Last 1
    if ($daily) {
      & $condaExe run -n DATA_C python scripts\build_llm_input.py --config $resolvedConfig --daily $daily.FullName --pattern (Join-Path $OutputDir "pattern_summary.json") --output (Join-Path $OutputDir "llm_input.json") --max-bytes $MaxBytes --store-db
      & $condaExe run -n DATA_C python scripts\generate_recommendations.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output-md (Join-Path $OutputDir "activity_recommendations.md") --output-json (Join-Path $OutputDir "activity_recommendations.json")
      & $condaExe run -n DATA_C python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output (Join-Path $OutputDir "n8n_workflow.json")
    }
    exit $LASTEXITCODE
  }
}

python scripts\build_daily_summary.py --config $resolvedConfig --store-db
python scripts\build_pattern_summary.py --summaries-dir $OutputDir --since-days 7 --config $resolvedConfig --store-db
python scripts\learn_profile.py --config $resolvedConfig --output configs\profile_learned.json --days 14
$dailyLocal = Get-ChildItem $OutputDir -Filter "daily_summary_*.json" | Sort-Object LastWriteTime | Select-Object -Last 1
if ($dailyLocal) {
  python scripts\build_llm_input.py --config $resolvedConfig --daily $dailyLocal.FullName --pattern (Join-Path $OutputDir "pattern_summary.json") --output (Join-Path $OutputDir "llm_input.json") --max-bytes $MaxBytes --store-db
  python scripts\generate_recommendations.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output-md (Join-Path $OutputDir "activity_recommendations.md") --output-json (Join-Path $OutputDir "activity_recommendations.json")
  python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input.json") --output (Join-Path $OutputDir "n8n_workflow.json")
}
