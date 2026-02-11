# Cleanup script for Steer backend
# Kills all running instances and removes lock file

Write-Host "🧹 Cleaning up Steer processes..." -ForegroundColor Yellow

# Kill all local_os_agent processes
$processes = Get-Process -Name "local_os_agent" -ErrorAction SilentlyContinue
if ($processes) {
    foreach ($proc in $processes) {
        Write-Host "  Killing PID: $($proc.Id)" -ForegroundColor Red
        Stop-Process -Id $proc.Id -Force
    }
    Write-Host "✅ Killed $($processes.Count) process(es)" -ForegroundColor Green
} else {
    Write-Host "  No running processes found" -ForegroundColor Gray
}

# Remove lock file
$lockFile = "$env:HOME\.steer\steer.lock"
if (Test-Path $lockFile) {
    Remove-Item $lockFile -Force
    Write-Host "✅ Removed lock file" -ForegroundColor Green
} else {
    Write-Host "  No lock file found" -ForegroundColor Gray
}

Write-Host ""
Write-Host "🚀 Ready to start! Run: cargo run --bin local_os_agent" -ForegroundColor Cyan
