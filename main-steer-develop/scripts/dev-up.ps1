param(
  [int]$CorePort = 5680,
  [int]$WebPort = 5173
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$RuntimeDir = Join-Path $RootDir ".runtime"
$LogDir = Join-Path $RuntimeDir "logs"
$PidFile = Join-Path $RuntimeDir "dev-processes.json"
$CoreDir = Join-Path $RootDir "core"
$WebDir = Join-Path $RootDir "web"
$SteerHome = Join-Path $RuntimeDir "steer_home"
$SteerDb = Join-Path $SteerHome "steer.db"

New-Item -ItemType Directory -Force -Path $RuntimeDir | Out-Null
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $SteerHome | Out-Null

function Test-HttpReady {
  param(
    [Parameter(Mandatory = $true)][string]$Url,
    [int]$TimeoutSeconds = 60
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  while ((Get-Date) -lt $deadline) {
    try {
      $resp = Invoke-WebRequest -Uri $Url -Method Get -UseBasicParsing -TimeoutSec 3
      if ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 500) {
        return $true
      }
    } catch {
      Start-Sleep -Milliseconds 700
    }
  }
  return $false
}

function Get-PidByPort {
  param([int]$Port)

  try {
    $conn = Get-NetTCPConnection -LocalAddress "127.0.0.1" -LocalPort $Port -State Listen -ErrorAction Stop | Select-Object -First 1
    if ($conn -and $conn.OwningProcess) {
      return [int]$conn.OwningProcess
    }
  } catch {}

  try {
    $line = netstat -ano | Select-String -Pattern (":$Port\s+.*LISTENING\s+\d+$") | Select-Object -First 1
    if ($line) {
      $parts = ($line.ToString() -split "\s+") | Where-Object { $_ -ne "" }
      if ($parts.Count -ge 5) {
        return [int]$parts[-1]
      }
    }
  } catch {}

  return $null
}

function Start-LoggedProcess {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$FilePath,
    [Parameter(Mandatory = $true)][string[]]$Arguments,
    [Parameter(Mandatory = $true)][string]$WorkingDirectory,
    [hashtable]$Environment = @{}
  )

  $outLog = Join-Path $LogDir ($Name + ".out.log")
  $errLog = Join-Path $LogDir ($Name + ".err.log")

  $previous = @{}
  foreach ($k in $Environment.Keys) {
    $previous[$k] = [Environment]::GetEnvironmentVariable($k, "Process")
    [Environment]::SetEnvironmentVariable($k, [string]$Environment[$k], "Process")
  }

  try {
    $proc = Start-Process `
      -FilePath $FilePath `
      -ArgumentList $Arguments `
      -WorkingDirectory $WorkingDirectory `
      -RedirectStandardOutput $outLog `
      -RedirectStandardError $errLog `
      -WindowStyle Hidden `
      -PassThru
  } finally {
    foreach ($k in $previous.Keys) {
      [Environment]::SetEnvironmentVariable($k, $previous[$k], "Process")
    }
  }

  return @{
    pid = $proc.Id
    out_log = $outLog
    err_log = $errLog
    started_at = (Get-Date).ToString("s")
  }
}

$coreHealthUrl = "http://127.0.0.1:$CorePort/api/health"
$webUrl = "http://127.0.0.1:$WebPort"

$state = @{
  core = $null
  web = $null
}

if (Test-HttpReady -Url $coreHealthUrl -TimeoutSeconds 2) {
  Write-Host "[i] Core already healthy at $coreHealthUrl"
  $existingCorePid = Get-PidByPort -Port $CorePort
  if ($existingCorePid) {
    $state.core = @{
      pid = $existingCorePid
      out_log = $null
      err_log = $null
      started_at = (Get-Date).ToString("s")
    }
  }
} else {
  Write-Host "[i] Starting core on port $CorePort"
  $coreEnv = @{
    STEER_ALLOW_MULTI = "1"
    STEER_DAEMON = "1"
    STEER_API_PORT = [string]$CorePort
    STEER_HOME = $SteerHome
    STEER_DB_PATH = $SteerDb
  }
  $state.core = Start-LoggedProcess `
    -Name "core" `
    -FilePath "cargo" `
    -Arguments @("run", "--release", "--bin", "local_os_agent") `
    -WorkingDirectory $CoreDir `
    -Environment $coreEnv

  if (-not (Test-HttpReady -Url $coreHealthUrl -TimeoutSeconds 120)) {
    Write-Host "[X] Core did not become healthy. Check logs:"
    Write-Host "    $($state.core.out_log)"
    Write-Host "    $($state.core.err_log)"
    exit 1
  }
  Write-Host "[OK] Core healthy at $coreHealthUrl"
}

if (Test-HttpReady -Url $webUrl -TimeoutSeconds 2) {
  Write-Host "[i] Web already serving at $webUrl"
  $existingWebPid = Get-PidByPort -Port $WebPort
  if ($existingWebPid) {
    $state.web = @{
      pid = $existingWebPid
      out_log = $null
      err_log = $null
      started_at = (Get-Date).ToString("s")
    }
  }
} else {
  Write-Host "[i] Starting web on port $WebPort"
  $state.web = Start-LoggedProcess `
    -Name "web" `
    -FilePath "npm.cmd" `
    -Arguments @("run", "dev", "--", "--host", "127.0.0.1", "--port", [string]$WebPort, "--strictPort") `
    -WorkingDirectory $WebDir

  if (-not (Test-HttpReady -Url $webUrl -TimeoutSeconds 90)) {
    Write-Host "[X] Web did not become ready. Check logs:"
    Write-Host "    $($state.web.out_log)"
    Write-Host "    $($state.web.err_log)"
    exit 1
  }
  Write-Host "[OK] Web ready at $webUrl"
}

$persist = @{
  core = $state.core
  web = $state.web
  core_url = $coreHealthUrl
  web_url = $webUrl
  saved_at = (Get-Date).ToString("s")
}
$persist | ConvertTo-Json -Depth 8 | Out-File -FilePath $PidFile -Encoding UTF8

Write-Host ""
Write-Host "[OK] Dev stack ready"
Write-Host "    Core: $coreHealthUrl"
Write-Host "    Web : $webUrl"
Write-Host "    PID : $PidFile"
Write-Host "    Logs: $LogDir"
