param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$ConfigPath = "configs\config.yaml",
  [string]$ProfilePath = "configs\personalization_demo.json",
  [string]$ScoreConfig = "configs\score_config.json",
  [int]$EverySeconds = 600,
  [int]$WindowMinutes = 10,
  [switch]$Once
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}

$resolvedProfile = $ProfilePath
if (-not (Test-Path $resolvedProfile)) {
  $resolvedProfile = Join-Path $RepoPath $ProfilePath
}

$resolvedScore = $ScoreConfig
if (-not (Test-Path $resolvedScore)) {
  $resolvedScore = Join-Path $RepoPath $ScoreConfig
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

while ($true) {
  python scripts\build_realtime_llm_input.py --config $resolvedConfig --since-minutes $WindowMinutes --output logs\llm_input_realtime.json --max-bytes 8000 --min-duration-sec 5 --drop-idle --summary-only
  python scripts\build_hybrid_llm_input.py --output logs\llm_input_hybrid.json --realtime logs\llm_input_realtime.json
  python scripts\generate_workflow_with_retry.py --config $resolvedConfig --input logs\llm_input_hybrid.json --output logs\n8n_workflow_hybrid.json --profile $resolvedProfile --score-config $resolvedScore --min-score 85 --max-attempts 3
  python scripts\score_n8n_workflow.py --file logs\n8n_workflow_hybrid.json --profile $resolvedProfile --score-config $resolvedScore --llm-input logs\llm_input_hybrid.json
  if ($Once) { break }
  Start-Sleep -Seconds $EverySeconds
}
