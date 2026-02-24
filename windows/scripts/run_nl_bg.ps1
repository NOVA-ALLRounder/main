param(
  [Parameter(Mandatory = $true, Position = 0)]
  [string]$RequestText,
  [Parameter(Position = 1)]
  [string]$TaskName = "Background NL Run"
)

$ErrorActionPreference = "Stop"

$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$runDir = Join-Path $rootDir "scenario_results\bg_runs"
New-Item -ItemType Directory -Path $runDir -Force | Out-Null

$ts = Get-Date -Format "yyyyMMdd_HHmmss"
$runId = "bg_$ts"
$pidFile = Join-Path $runDir "$runId.pid"
$logFile = Join-Path $runDir "$runId.driver.log"
$metaFile = Join-Path $runDir "$runId.meta"
$latestFile = Join-Path $runDir "latest_run_id"

$meta = @(
  "run_id=$runId"
  "request=$RequestText"
  "task=$TaskName"
  "created_at=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
)
Set-Content -Path $metaFile -Value $meta

if (-not $env:STEER_PAUSE_ON_USER_INPUT) { $env:STEER_PAUSE_ON_USER_INPUT = "1" }
if (-not $env:STEER_USER_ACTIVE_APPS) { $env:STEER_USER_ACTIVE_APPS = "Terminal,Codex,iTerm2" }
if (-not $env:STEER_INPUT_ACTIVE_THRESHOLD_SECONDS) { $env:STEER_INPUT_ACTIVE_THRESHOLD_SECONDS = "1" }
if (-not $env:STEER_IDLE_RESUME_SECONDS) { $env:STEER_IDLE_RESUME_SECONDS = "3" }
if (-not $env:STEER_INPUT_POLL_SECONDS) { $env:STEER_INPUT_POLL_SECONDS = "1" }
if (-not $env:STEER_REQUIRE_TERMINAL) { $env:STEER_REQUIRE_TERMINAL = "0" }
if (-not $env:STEER_NODE_CAPTURE_ALL) { $env:STEER_NODE_CAPTURE_ALL = "1" }

$driverScript = Join-Path $rootDir "run_nl_request_with_telegram.ps1"
if (-not (Test-Path $driverScript)) {
  throw "Driver script not found: $driverScript"
}

$proc = Start-Process -FilePath "powershell.exe" `
  -ArgumentList @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $driverScript,
    $RequestText,
    $TaskName
  ) `
  -WorkingDirectory $rootDir `
  -WindowStyle Hidden `
  -RedirectStandardOutput $logFile `
  -RedirectStandardError $logFile `
  -PassThru

Set-Content -Path $pidFile -Value $proc.Id
Set-Content -Path $latestFile -Value $runId

Write-Output "started_run_id=$runId"
Write-Output "pid=$($proc.Id)"
Write-Output "driver_log=$logFile"
Write-Output "pid_file=$pidFile"
