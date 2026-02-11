param(
  [string]$TaskName = "DataCollectorArchive"
)

$ErrorActionPreference = "Stop"

if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
  Write-Host "Task removed: $TaskName"
} else {
  Write-Host "Task not found: $TaskName"
}
