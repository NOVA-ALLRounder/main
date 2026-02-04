param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [string]$WatchPath = "C:\\collector_test",
  [int]$PollSeconds = 1,
  [int]$IdleThreshold = 10
)

$ErrorActionPreference = "Stop"
$env:PYTHONPATH = "src"
Set-Location $RepoPath

$dcpRoot = Join-Path $RepoPath "collector\\Data-Collection-Projection"
$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $dcpRoot $ConfigPath
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
