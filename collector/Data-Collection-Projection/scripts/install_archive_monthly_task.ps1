param(
  [string]$TaskName = "DataCollectorArchiveMonthly",
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ArchiveDir = "archive\\raw",
  [string]$MonthlyDir = "archive\\monthly",
  [string]$Day = "1",
  [string]$Time = "03:30",
  [switch]$DeleteAfter,
  [switch]$RunAsHighest
)

$ErrorActionPreference = "Stop"

$runScript = Join-Path $RepoPath "scripts\\archive_monthly.ps1"
if (-not (Test-Path $runScript)) {
  throw "archive_monthly.ps1 not found at $runScript"
}

$deleteArg = ""
if ($DeleteAfter) { $deleteArg = "-DeleteAfter" }

$arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$runScript`" -RepoPath `"$RepoPath`" -ArchiveDir `"$ArchiveDir`" -MonthlyDir `"$MonthlyDir`" $deleteArg"

$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $arguments
$trigger = New-ScheduledTaskTrigger -Monthly -DaysOfMonth $Day -At $Time

$settings = New-ScheduledTaskSettingsSet `
  -StartWhenAvailable `
  -MultipleInstances IgnoreNew `
  -ExecutionTimeLimit (New-TimeSpan -Hours 2) `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries

if ($RunAsHighest) {
  $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest
  Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
} else {
  Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
}

Write-Host "Task registered: $TaskName"
