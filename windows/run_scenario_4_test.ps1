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

$request = if ($Args.Count -gt 0) {
  ($Args -join " ")
} else {
  "Open File Explorer, list files in Downloads, and read the first filename."
}

New-Item -ItemType Directory -Force -Path "scenario_results" | Out-Null
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$logFile = Join-Path "scenario_results" "scenario_4_$timestamp.log"

Write-Host "Scenario 4: File Explorer navigation"
Write-Host "Logs: $logFile"

try {
  & cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf $request *> $logFile
  if ($LASTEXITCODE -ne 0) {
    throw "cargo run failed with code $LASTEXITCODE"
  }

  Write-Host "Scenario 4 complete."
  $message = "Scenario 4 complete.`nTime: $(Get-Date -Format 'HH:mm:ss')"
  & (Join-Path $PSScriptRoot "send_telegram_notification.ps1") $message
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "Telegram notification failed with code $LASTEXITCODE"
  }
  exit 0
} catch {
  Write-Host "Scenario 4 failed."
  $summary = ""
  if (Test-Path $logFile) {
    $summary = (Get-Content -Path $logFile -Tail 20) -join "`n"
  }
  $message = "Scenario 4 failed.`n$summary"
  & (Join-Path $PSScriptRoot "send_telegram_notification.ps1") $message
  exit 1
}
