param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location $PSScriptRoot

function Load-DotEnvFile {
  param([Parameter(Mandatory = $true)][string]$Path)
  if (-not (Test-Path $Path)) {
    return
  }
  foreach ($line in Get-Content -Path $Path) {
    $trimmed = $line.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
      continue
    }
    $idx = $trimmed.IndexOf("=")
    if ($idx -lt 1) {
      continue
    }
    $name = $trimmed.Substring(0, $idx).Trim()
    $value = $trimmed.Substring($idx + 1).Trim()
    if (($value.StartsWith('"') -and $value.EndsWith('"')) -or ($value.StartsWith("'") -and $value.EndsWith("'"))) {
      $value = $value.Substring(1, $value.Length - 2)
    }
    [Environment]::SetEnvironmentVariable($name, $value)
  }
}

Load-DotEnvFile -Path "core/.env"
New-Item -ItemType Directory -Force -Path "scenario_results" | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$logFile = Join-Path "scenario_results" "complex_scenario_5_$timestamp.log"

$defaultRequest = @"
Open Notes and create a new note with title "Research Topic: Rust programming language".
Type "Rust programming", copy it, open Microsoft Edge, paste and search.
Copy the first result URL, return to Notes, paste it, select all and copy.
Open Outlook, create a new mail with subject "Research Findings", paste the note body, and send.
"@

$request = if ($Args.Count -gt 0) {
  ($Args -join " ")
} else {
  $defaultRequest.Trim()
}

Write-Host "Starting scenario 5 execution..."
Write-Host "Please do not touch mouse/keyboard during execution."
Write-Host "Logs: $logFile"

try {
  & cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf $request *> $logFile
  if ($LASTEXITCODE -ne 0) {
    throw "cargo run failed with code $LASTEXITCODE"
  }

  Write-Host "Scenario 5 complete."
  $summary = (Get-Content -Path $logFile -Tail 50) -join "`n"
  $message = "Scenario 5 complete.`n$summary"
  & (Join-Path $PSScriptRoot "send_telegram_notification.ps1") $message
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "Telegram notification failed with code $LASTEXITCODE"
  }
  exit 0
} catch {
  Write-Host "Scenario 5 failed."
  $summary = ""
  if (Test-Path $logFile) {
    $summary = (Get-Content -Path $logFile -Tail 30) -join "`n"
  }
  $message = "Scenario 5 failed.`n$summary"
  & (Join-Path $PSScriptRoot "send_telegram_notification.ps1") $message
  exit 1
}
