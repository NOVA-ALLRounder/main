param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config_run4.yaml",
  [string]$OutputDir = "logs\\run4",
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

while ($true) {
  if (-not $NoConda) {
    $conda = Get-Command conda -ErrorAction SilentlyContinue
    if ($conda) {
      $condaExe = $conda.Path
      if (-not $condaExe) { $condaExe = $conda.Source }
      if (-not $condaExe) { $condaExe = "conda" }
      & $condaExe run -n DATA_C python scripts\build_realtime_llm_input.py --config $resolvedConfig --since-minutes $WindowMinutes --output (Join-Path $OutputDir "llm_input_realtime.json") --max-bytes $MaxBytes
      & $condaExe run -n DATA_C python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input_realtime.json") --output (Join-Path $OutputDir "n8n_workflow_realtime.json")
      Start-Sleep -Seconds $EverySeconds
      continue
    }
  }

  python scripts\build_realtime_llm_input.py --config $resolvedConfig --since-minutes $WindowMinutes --output (Join-Path $OutputDir "llm_input_realtime.json") --max-bytes $MaxBytes
  python scripts\generate_n8n_workflow.py --config $resolvedConfig --input (Join-Path $OutputDir "llm_input_realtime.json") --output (Join-Path $OutputDir "n8n_workflow_realtime.json")
  Start-Sleep -Seconds $EverySeconds
}
