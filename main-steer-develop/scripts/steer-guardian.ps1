$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$coreDir = Resolve-Path (Join-Path $scriptDir "..\apps\core")

$logRoot = Join-Path $env:USERPROFILE ".steer\logs"
if (-not $logRoot) {
    $logRoot = Join-Path (Get-Location) ".steer\logs"
}
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null

$guardianLog = Join-Path $logRoot "guardian.log"
$crashLog = Join-Path $logRoot "crash.log"

$binCandidates = @(
    Join-Path $coreDir "target\release\local_os_agent.exe",
    Join-Path $coreDir "target\release\core.exe",
    Join-Path $coreDir "target\release\steer-core.exe"
)

$bin = $binCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $bin) {
    Write-Host "❌ Binary not found. Run 'cargo build --release' in apps/core first."
    exit 1
}

Write-Host "🛡️  Steer Guardian Active. Monitoring $bin..."
Write-Host "------------------------------------------------"

while ($true) {
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -Path $guardianLog -Value "[$timestamp] Starting core..."

    & $bin
    $exitCode = $LASTEXITCODE

    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -Path $guardianLog -Value "[$timestamp] Process exited with code: $exitCode"

    if ($exitCode -eq 0) {
        Write-Host "✅ Core exited normally (user quit). Guardian shutting down."
        break
    } else {
        Write-Host "⚠️  CRASH DETECTED (Code $exitCode). Restarting in 3 seconds..."
        Add-Content -Path $guardianLog -Value "[$timestamp] Crash detected (code $exitCode). See $crashLog for details."
        Start-Sleep -Seconds 3
    }
}

Write-Host "🛡️  Guardian Terminated."
