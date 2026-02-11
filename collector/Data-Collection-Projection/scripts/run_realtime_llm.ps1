param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [string]$OutputDir = "logs",
  [int]$EverySeconds = 300,
  [int]$WindowMinutes = 10,
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

while ($true) {
  if (-not $NoConda) {
    $conda = Get-Command conda -ErrorAction SilentlyContinue
    if ($conda) {
      $condaExe = $conda.Path
      if (-not $condaExe) { $condaExe = $conda.Source }
      if (-not $condaExe) { $condaExe = "conda" }
      & $condaExe run -n DATA_C python scripts\build_realtime_llm_input.py --config $resolvedConfig --since-minutes $WindowMinutes --output (Join-Path $OutputDir "llm_input_realtime.json") --max-bytes $MaxBytes --summary-only
      & $condaExe run -n DATA_C python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input_realtime.json") --output (Join-Path $OutputDir "n8n_workflow_realtime.json")
      Start-Sleep -Seconds $EverySeconds
      continue
    }
  }

  python scripts\build_realtime_llm_input.py --config $resolvedConfig --since-minutes $WindowMinutes --output (Join-Path $OutputDir "llm_input_realtime.json") --max-bytes $MaxBytes --summary-only
  python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input_realtime.json") --output (Join-Path $OutputDir "n8n_workflow_realtime.json")
  Start-Sleep -Seconds $EverySeconds
}
