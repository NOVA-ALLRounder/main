param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path
)

$ErrorActionPreference = "Stop"

$coreDir = Join-Path $RepoPath "core"
$manifestPath = Join-Path $coreDir "Cargo.toml"
$binPath = Join-Path $coreDir "target\debug\local_os_agent.exe"

$logDir = Join-Path $env:USERPROFILE ".local-os-agent"
$logPath = Join-Path $logDir "manual_agent.log"
$errPath = Join-Path $logDir "manual_agent.err.log"
$pidPath = Join-Path $logDir "manual_agent.pid"
$lockDir = Join-Path $env:USERPROFILE ".steer\locks"

$statusUrl = "http://127.0.0.1:5680/api/status"
$preflightUrl = "http://127.0.0.1:5680/api/agent/preflight"

function Stop-ConflictingProcesses {
  if (Test-Path $pidPath) {
    $oldPid = (Get-Content $pidPath -ErrorAction SilentlyContinue | Select-Object -First 1)
    if ($oldPid -as [int]) {
      Stop-Process -Id ([int]$oldPid) -Force -ErrorAction SilentlyContinue
    }
  }

  Get-Process -Name "local_os_agent" -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue

  foreach ($port in @(5680)) {
    $conns = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
    $pids = $conns | Select-Object -ExpandProperty OwningProcess -Unique
    foreach ($pid in $pids) {
      if ($pid -as [int]) {
        Stop-Process -Id ([int]$pid) -Force -ErrorAction SilentlyContinue
      }
    }
  }

  Start-Sleep -Seconds 1
}

function Cleanup-StaleLocks {
  if (-not (Test-Path $lockDir)) { return }
  Get-ChildItem -Path $lockDir -Filter "steer.*.lock" -File -ErrorAction SilentlyContinue | ForEach-Object {
    $ownerPid = $null
    try {
      $raw = Get-Content $_.FullName -Raw -ErrorAction Stop
      $obj = $raw | ConvertFrom-Json -ErrorAction Stop
      if ($obj.pid) { $ownerPid = [int]$obj.pid }
    } catch {
      $ownerPid = $null
    }

    if (-not $ownerPid -or -not (Get-Process -Id $ownerPid -ErrorAction SilentlyContinue)) {
      Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
    }
  }
}

function Wait-ForHealth {
  param([int]$MaxSeconds = 40, [int]$Pid)
  for ($i = 0; $i -lt $MaxSeconds; $i++) {
    if (-not (Get-Process -Id $Pid -ErrorAction SilentlyContinue)) {
      return $false
    }
    try {
      $resp = Invoke-WebRequest -Uri $statusUrl -UseBasicParsing -TimeoutSec 2
      if ($resp.StatusCode -eq 200) { return $true }
    } catch {}
    Start-Sleep -Seconds 1
  }
  return $false
}

Write-Host "[1/6] Building debug runtime binary..."
& cargo build --manifest-path $manifestPath --bin local_os_agent | Out-Null
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "[2/6] Stopping conflicting agent processes..."
Stop-ConflictingProcesses

Write-Host "[3/6] Cleaning stale lock files..."
Cleanup-StaleLocks

Write-Host "[4/6] Starting local agent in background..."
New-Item -ItemType Directory -Path $logDir -Force | Out-Null
Set-Content -Path $logPath -Value "" -NoNewline
Set-Content -Path $errPath -Value "" -NoNewline

$previous = @{
  STEER_API_ALLOW_NO_KEY = $env:STEER_API_ALLOW_NO_KEY
  STEER_DISABLE_EVENT_TAP = $env:STEER_DISABLE_EVENT_TAP
  STEER_COLLECTOR_HANDOFF_AUTOCONSUME = $env:STEER_COLLECTOR_HANDOFF_AUTOCONSUME
}
$env:STEER_API_ALLOW_NO_KEY = "1"
$env:STEER_DISABLE_EVENT_TAP = "1"
$env:STEER_COLLECTOR_HANDOFF_AUTOCONSUME = "0"

$proc = Start-Process -FilePath $binPath `
  -WorkingDirectory $coreDir `
  -RedirectStandardOutput $logPath `
  -RedirectStandardError $errPath `
  -PassThru

foreach ($k in $previous.Keys) {
  if ($null -eq $previous[$k]) {
    Remove-Item "Env:$k" -ErrorAction SilentlyContinue
  } else {
    Set-Item "Env:$k" -Value $previous[$k]
  }
}

Set-Content -Path $pidPath -Value $proc.Id

Write-Host "[5/6] Waiting for API health..."
if (-not (Wait-ForHealth -Pid $proc.Id)) {
  Write-Host "Runtime recovery failed."
  Write-Host "  - pid: $($proc.Id)"
  Write-Host "  - log: $logPath"
  Get-Content $logPath -Tail 120 -ErrorAction SilentlyContinue
  exit 1
}

Write-Host "[6/6] Stability check..."
Start-Sleep -Seconds 8
if (-not (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue)) {
  Write-Host "Runtime recovery failed (process exited)."
  Get-Content $logPath -Tail 120 -ErrorAction SilentlyContinue
  exit 1
}

$statusCode = "000"
$preflightCode = "000"
try {
  $statusCode = (Invoke-WebRequest -Uri $statusUrl -UseBasicParsing -TimeoutSec 3).StatusCode
} catch {}
try {
  $preflightCode = (Invoke-WebRequest -Uri $preflightUrl -UseBasicParsing -TimeoutSec 3).StatusCode
} catch {}

Write-Host "Recovery complete."
Write-Host "Agent PID: $($proc.Id)"
Write-Host "/api/status: $statusCode"
Write-Host "/api/agent/preflight: $preflightCode"
Write-Host "Log file: $logPath"
