param(
  [int[]]$Ports = @(5680, 5174, 5173)
)

$ErrorActionPreference = "Stop"

Write-Host "Killing processes..."

$killed = New-Object System.Collections.Generic.HashSet[int]

foreach ($port in $Ports) {
  $connections = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
  $pids = $connections | Select-Object -ExpandProperty OwningProcess -Unique
  if (-not $pids) {
    Write-Host "  - Port $port is free."
    continue
  }

  foreach ($pid in $pids) {
    if ($pid -le 0) { continue }
    try {
      Stop-Process -Id $pid -Force -ErrorAction Stop
      [void]$killed.Add($pid)
      Write-Host "  - Killed PID $pid on port $port."
    } catch {
      Write-Host "  - Failed to kill PID $pid on port ${port}: $($_.Exception.Message)"
    }
  }
}

$named = Get-Process -ErrorAction SilentlyContinue |
  Where-Object { $_.ProcessName -ieq "local_os_agent" }

foreach ($p in $named) {
  if ($killed.Contains($p.Id)) { continue }
  try {
    Stop-Process -Id $p.Id -Force -ErrorAction Stop
    Write-Host "  - Killed local_os_agent PID $($p.Id)."
  } catch {
    Write-Host "  - Failed to kill local_os_agent PID $($p.Id): $($_.Exception.Message)"
  }
}

Write-Host "All clean."
