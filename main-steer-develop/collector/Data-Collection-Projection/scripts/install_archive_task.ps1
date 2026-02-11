param(
  [string]$TaskName = "DataCollectorArchive",
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config_run4.yaml",
  [string]$ArchiveDir = "archive\\raw",
  [string]$Time = "03:00",
  [int]$Days = 1,
  [switch]$DeleteAfter,
  [switch]$RunAsHighest
)

$ErrorActionPreference = "Stop"

$runScript = Join-Path $RepoPath "scripts\\archive_daily.ps1"
if (-not (Test-Path $runScript)) {
  throw "archive_daily.ps1 not found at $runScript"
}

$deleteArg = ""
if ($DeleteAfter) { $deleteArg = "-DeleteAfter" }

$arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$runScript`" -RepoPath `"$RepoPath`" -ConfigPath `"$ConfigPath`" -ArchiveDir `"$ArchiveDir`" -Days $Days $deleteArg"

$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $arguments
$trigger = New-ScheduledTaskTrigger -Daily -At $Time

$settings = New-ScheduledTaskSettingsSet `
  -StartWhenAvailable `
  -MultipleInstances IgnoreNew `
  -ExecutionTimeLimit (New-TimeSpan -Hours 1) `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries

if ($RunAsHighest) {
  $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -RunLevel Highest
  Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
} else {
  Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
}

Write-Host "Task registered: $TaskName"
