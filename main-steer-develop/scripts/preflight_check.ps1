param(
    [string]$ApiBase = "http://127.0.0.1:5680/api"
)

$ErrorActionPreference = "Stop"

Write-Host "Running Steer preflight check..." -ForegroundColor Cyan

try {
    $health = Invoke-RestMethod -Method Get -Uri "$ApiBase/system/health"
    $mode = Invoke-RestMethod -Method Get -Uri "$ApiBase/system/mode"
    $preflight = Invoke-RestMethod -Method Get -Uri "$ApiBase/system/preflight"
} catch {
    Write-Host "FAILED: API unreachable at $ApiBase" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor DarkRed
    exit 1
}

Write-Host ""
Write-Host "API Port           : $($preflight.api_port)"
Write-Host "API Reachable      : $($preflight.api_reachable)"
Write-Host "Operation Mode     : $($mode.mode)"
Write-Host "Emergency Stop     : $($mode.emergency_stop)"
Write-Host "Allow Automation   : $($mode.allow_automation)"
Write-Host "Env Present        : $($preflight.env_present)"
Write-Host "STEER_HOME Writable: $($preflight.steer_home_writable)"
Write-Host "Release Writable   : $($preflight.release_dir_writable)"
Write-Host "Gmail Credentials  : $($preflight.gmail_credentials_set)"
Write-Host "Notion Ready       : $($preflight.notion_ready)"

if ($preflight.notes -and $preflight.notes.Count -gt 0) {
    Write-Host ""
    Write-Host "Notes:" -ForegroundColor Yellow
    foreach ($n in $preflight.notes) {
        Write-Host " - $n"
    }
}

Write-Host ""
if ($preflight.ok) {
    Write-Host "Preflight: PASS" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Preflight: WARN/FAIL (see notes)" -ForegroundColor Yellow
    exit 2
}
