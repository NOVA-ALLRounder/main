Param(
  [string]$CondaEnv = "DATA_C",
  [string]$ConfigPath = "configs\\config_run5.yaml",
  [string]$KeyPath = "secrets\\collector_key.txt"
)

$ErrorActionPreference = "Stop"

if (Test-Path $KeyPath) {
  $env:DATA_COLLECTOR_ENC_KEY = (Get-Content $KeyPath -Raw).Trim()
}

$env:PYTHONPATH = "src"

Write-Host "Starting collector with $ConfigPath"
python -m collector.main --config $ConfigPath
