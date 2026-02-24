param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [int]$RestartDelaySec = 3
)

$ErrorActionPreference = "Stop"

$coreBin = Join-Path $RepoPath "core\target\release\steer-core.exe"
$logDir = Join-Path $env:USERPROFILE ".steer\logs"
$guardianLog = Join-Path $logDir "guardian.log"

New-Item -ItemType Directory -Path $logDir -Force | Out-Null

if (-not (Test-Path $coreBin)) {
  throw "steer-core binary not found: $coreBin"
}

Write-Host "Steer Guardian Active. Monitoring steer-core..."
Write-Host "------------------------------------------------"

while ($true) {
  $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
  Add-Content -Path $guardianLog -Value "[$ts] Starting steer-core..."

  $proc = Start-Process -FilePath $coreBin -WorkingDirectory (Split-Path $coreBin) -PassThru -Wait
  $exitCode = $proc.ExitCode

  $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
  Add-Content -Path $guardianLog -Value "[$ts] Process exited with code: $exitCode"

  if ($exitCode -eq 0) {
    Write-Host "Steer Core exited normally. Guardian shutting down."
    break
  }

  Write-Host "Crash detected (Code $exitCode). Restarting in $RestartDelaySec seconds..."
  Start-Sleep -Seconds $RestartDelaySec
}

Write-Host "Guardian terminated."
