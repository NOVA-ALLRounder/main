Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$RuntimeDir = Join-Path $RootDir ".runtime"
$PidFile = Join-Path $RuntimeDir "dev-processes.json"

function Stop-ProcessSafe {
  param(
    [int]$TargetPid,
    [string]$Name
  )
  if ($TargetPid -le 0) { return }
  try {
    $p = Get-Process -Id $TargetPid -ErrorAction Stop
    $null = & taskkill /PID $TargetPid /T /F 2>$null
    Write-Host "[OK] Stopped $Name (pid=$TargetPid, proc=$($p.ProcessName), tree=true)"
  } catch {
    Write-Host "[i] $Name pid=$TargetPid not running"
  }
}

if (-not (Test-Path $PidFile)) {
  Write-Host "[i] No pid file at $PidFile"
  exit 0
}

try {
  $state = Get-Content $PidFile -Raw | ConvertFrom-Json
} catch {
  Write-Host "[WARN] Failed to read pid file, deleting: $PidFile"
  Remove-Item -Force $PidFile -ErrorAction SilentlyContinue
  exit 0
}

if ($state.core -and $state.core.pid) {
  Stop-ProcessSafe -TargetPid ([int]$state.core.pid) -Name "core"
}
if ($state.web -and $state.web.pid) {
  Stop-ProcessSafe -TargetPid ([int]$state.web.pid) -Name "web"
}

Remove-Item -Force $PidFile -ErrorAction SilentlyContinue
Write-Host "[OK] Dev stack stopped"
