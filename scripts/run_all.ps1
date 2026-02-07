param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [string]$WatchPath = "C:\\collector_test",
  [int]$PollSeconds = 1,
  [int]$IdleThreshold = 10,
  [switch]$SelectAllowlist,
  [string]$SelectionPath = "configs\\allowlist_selection.yaml",
  [switch]$IncludeInstalled,
  [switch]$IncludeRunning
)

$ErrorActionPreference = "Stop"
$env:PYTHONPATH = "src"
Set-Location $RepoPath

$dcpRoot = Join-Path $RepoPath "collector\\Data-Collection-Projection"
$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $dcpRoot $ConfigPath
}

# Ensure encryption key is set (required when encryption enabled)
if (-not $env:DATA_COLLECTOR_ENC_KEY) {
  $keyPath = Join-Path $dcpRoot "secrets\\collector_key.txt"
  if (Test-Path $keyPath) {
    $env:DATA_COLLECTOR_ENC_KEY = (Get-Content $keyPath -Raw).Trim()
  } else {
    $envPath = Join-Path $RepoPath ".env"
    if (Test-Path $envPath) {
      $line = Get-Content $envPath | Where-Object { $_ -match '^DATA_COLLECTOR_ENC_KEY=' } | Select-Object -First 1
      if ($line) {
        $env:DATA_COLLECTOR_ENC_KEY = ($line -split '=',2)[1].Trim()
      }
    }
  }
}

# Optional allowlist selection
if ($SelectAllowlist) {
  $resolvedSelection = $SelectionPath
  if (-not (Test-Path $resolvedSelection)) {
    $resolvedSelection = Join-Path $dcpRoot $SelectionPath
  }
  $allowlistArgs = @("--config", $resolvedConfig, "--output", $resolvedSelection, "--include-observed")
  if ($IncludeInstalled) { $allowlistArgs += "--include-installed" }
  if ($IncludeRunning) { $allowlistArgs += "--include-running" }

  Write-Host "▶ Building allowlist selection template..."
  conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\allowlist_wizard.py") @allowlistArgs

  Write-Host "▶ Edit allowlist selection file: $resolvedSelection"
  Start-Process -FilePath "notepad.exe" -ArgumentList $resolvedSelection -WorkingDirectory $dcpRoot
  Read-Host "Press Enter after saving your allow/deny selection"

  Write-Host "▶ Applying allowlist selection..."
  conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\allowlist_wizard.py") --config $resolvedConfig --apply-selection $resolvedSelection
  Write-Host "✅ Allowlist selection applied (see output above)."
}

# Ensure DB/migrations are ready
conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\init_db.py") --config $resolvedConfig

# Start core collector
Start-Process -FilePath "conda" -ArgumentList @("run","-n","DATA_C","python","-m","collector.main","--config",$resolvedConfig) -WorkingDirectory $dcpRoot

# Start sensors
Start-Process -FilePath "conda" -ArgumentList @("run","-n","DATA_C","python","-m","sensors.os.windows_foreground","--ingest-url","http://127.0.0.1:8080/events","--poll",$PollSeconds) -WorkingDirectory $dcpRoot
Start-Process -FilePath "conda" -ArgumentList @("run","-n","DATA_C","python","-m","sensors.os.windows_idle","--ingest-url","http://127.0.0.1:8080/events","--idle-threshold",$IdleThreshold,"--poll",$PollSeconds) -WorkingDirectory $dcpRoot
Start-Process -FilePath "conda" -ArgumentList @("run","-n","DATA_C","python","-m","sensors.os.file_watcher","--ingest-url","http://127.0.0.1:8080/events","--paths",$WatchPath) -WorkingDirectory $dcpRoot

Write-Host "? DCP + sensors started"
