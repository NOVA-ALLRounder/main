param(
  [ValidateSet("status", "pause", "resume", "stop", "tail")]
  [string]$Action = "status",
  [string]$RunId
)

$ErrorActionPreference = "Stop"

$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$runDir = Join-Path $rootDir "scenario_results\bg_runs"
$latestFile = Join-Path $runDir "latest_run_id"

if (-not $RunId -and (Test-Path $latestFile)) {
  $RunId = (Get-Content $latestFile -ErrorAction SilentlyContinue | Select-Object -First 1)
}
if (-not $RunId) {
  throw "No run_id provided and no latest run found."
}

$pidFile = Join-Path $runDir "$RunId.pid"
$metaFile = Join-Path $runDir "$RunId.meta"
$logFile = Join-Path $runDir "$RunId.driver.log"

if (-not (Test-Path $pidFile)) {
  throw "PID file not found: $pidFile"
}

$pidRaw = Get-Content $pidFile -ErrorAction Stop | Select-Object -First 1
if (-not ($pidRaw -as [int])) {
  throw "Invalid PID in file: $pidFile"
}
$pid = [int]$pidRaw

function Get-TargetProcess {
  Get-Process -Id $pid -ErrorAction SilentlyContinue
}

switch ($Action) {
  "status" {
    Write-Output "run_id=$RunId"
    Write-Output "pid=$pid"
    if (Test-Path $metaFile) {
      Get-Content $metaFile
    }
    $proc = Get-TargetProcess
    if ($proc) {
      Write-Output "alive=1"
      Write-Output "state=running"
    } else {
      Write-Output "alive=0"
      Write-Output "state=exited"
    }
    Write-Output "driver_log=$logFile"
  }
  "pause" {
    $proc = Get-TargetProcess
    if (-not $proc) { throw "process not alive" }
    $suspend = Get-Command Suspend-Process -ErrorAction SilentlyContinue
    if (-not $suspend) {
      throw "Suspend-Process is not available in this PowerShell environment."
    }
    Suspend-Process -Id $pid -ErrorAction Stop
    Write-Output "paused run_id=$RunId pid=$pid"
  }
  "resume" {
    $proc = Get-TargetProcess
    if (-not $proc) { throw "process not alive" }
    $resume = Get-Command Resume-Process -ErrorAction SilentlyContinue
    if (-not $resume) {
      throw "Resume-Process is not available in this PowerShell environment."
    }
    Resume-Process -Id $pid -ErrorAction Stop
    Write-Output "resumed run_id=$RunId pid=$pid"
  }
  "stop" {
    $proc = Get-TargetProcess
    if ($proc) {
      Stop-Process -Id $pid -ErrorAction SilentlyContinue
      Start-Sleep -Seconds 1
      if (Get-TargetProcess) {
        Stop-Process -Id $pid -Force -ErrorAction SilentlyContinue
      }
      Write-Output "stopped run_id=$RunId pid=$pid"
    } else {
      Write-Output "process already exited"
    }
  }
  "tail" {
    if (-not (Test-Path $logFile)) {
      throw "driver log missing: $logFile"
    }
    Get-Content $logFile -Tail 80
  }
}
