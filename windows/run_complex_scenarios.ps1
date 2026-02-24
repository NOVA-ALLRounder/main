param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location $PSScriptRoot

function Get-EnvValue {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Default
  )
  $value = [Environment]::GetEnvironmentVariable($Name)
  if ([string]::IsNullOrWhiteSpace($value)) {
    return $Default
  }
  return $value
}

function Normalize-ScenarioIds {
  param([Parameter(Mandatory = $true)][string]$Raw)
  $result = New-Object System.Collections.Generic.List[string]
  foreach ($token in ($Raw -split '[,/\s]+')) {
    $trimmed = $token.Trim()
    if ($trimmed -in @("1", "2", "3", "4", "5")) {
      if (-not $result.Contains($trimmed)) {
        [void]$result.Add($trimmed)
      }
    }
  }
  return @($result)
}

function Get-MailProofStatusFromLog {
  param([Parameter(Mandatory = $true)][string]$LogPath)
  if (-not (Test-Path $LogPath)) {
    return "none"
  }
  $line = Get-Content -Path $LogPath | Where-Object { $_ -match 'MAIL_SEND_PROOF\|status=' } | Select-Object -Last 1
  if ([string]::IsNullOrWhiteSpace($line)) {
    return "none"
  }
  $m = [regex]::Match($line, 'status=([^|]+)')
  if ($m.Success -and -not [string]::IsNullOrWhiteSpace($m.Groups[1].Value)) {
    return $m.Groups[1].Value
  }
  return "none"
}

function Get-GitEmail {
  try {
    $email = (& git config --get user.email 2>$null)
    if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($email)) {
      return $email.Trim()
    }
  } catch {
    # no-op
  }
  return "test@example.com"
}

function Invoke-Scenario {
  param(
    [Parameter(Mandatory = $true)][int]$ScenarioId,
    [Parameter(Mandatory = $true)][string]$ScenarioName,
    [Parameter(Mandatory = $true)][string]$ScenarioGoal,
    [Parameter(Mandatory = $true)][string]$ScenarioRequest,
    [Parameter(Mandatory = $true)][string]$Timestamp,
    [Parameter(Mandatory = $true)][string]$RequireTelegramValue,
    [Parameter(Mandatory = $true)][string]$RunnerPath
  )

  Write-Host "---------------------------------------------------"
  Write-Host ("Scenario {0}: {1}" -f $ScenarioId, $ScenarioName)
  Write-Host ("Goal: {0}" -f $ScenarioGoal)

  $logFile = Join-Path "scenario_results" ("complex_scenario_{0}_{1}.log" -f $ScenarioId, $Timestamp)

  $outputLines = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $RunnerPath $ScenarioRequest $ScenarioName 2>&1
  $exitCode = $LASTEXITCODE
  if ($null -eq $outputLines) {
    "" | Set-Content -Path $logFile -Encoding UTF8
  } else {
    $outputLines | Set-Content -Path $logFile -Encoding UTF8
    $outputLines | ForEach-Object { Write-Output $_ }
  }

  $status = if ($exitCode -eq 0) { "success" } else { "failed" }
  $mailProofStatus = Get-MailProofStatusFromLog -LogPath $logFile
  $semanticMissing = if ($status -eq "success") { 0 } else { 1 }

  $attemptLine = "RUN_ATTEMPT|phase=scenario_{0}_final_judgement|status={1}|details=semantic_missing={2},mail_proof={3},node_capture_required=1,telegram_required={4}|ts={5}" -f `
    $ScenarioId, `
    $status, `
    $semanticMissing, `
    $mailProofStatus, `
    $RequireTelegramValue, `
    (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

  Add-Content -Path $logFile -Value $attemptLine -Encoding UTF8
  Write-Output $attemptLine

  if ($status -eq "success") {
    Write-Host ("Scenario {0} Complete." -f $ScenarioId)
    return $true
  }

  Write-Host ("Scenario {0} Failed." -f $ScenarioId)
  return $false
}

New-Item -ItemType Directory -Force -Path "scenario_results" | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$scenarioIdsRaw = Get-EnvValue -Name "STEER_SCENARIO_IDS" -Default "1,2,3,4,5"
$selectedScenarioIds = Normalize-ScenarioIds -Raw $scenarioIdsRaw
if ($selectedScenarioIds.Count -eq 0) {
  Write-Error "No valid scenario IDs selected. Allowed: 1,2,3,4,5"
  exit 1
}

$selectedScenarioCount = $selectedScenarioIds.Count
$scenarioModeValue = Get-EnvValue -Name "STEER_SCENARIO_MODE" -Default "0"
$nodeCaptureAllValue = Get-EnvValue -Name "STEER_NODE_CAPTURE_ALL" -Default "1"
$failOnFallbackValue = Get-EnvValue -Name "STEER_FAIL_ON_FALLBACK" -Default "1"
$testModeValue = Get-EnvValue -Name "STEER_TEST_MODE" -Default "0"
$requireTelegramReportValue = Get-EnvValue -Name "STEER_REQUIRE_TELEGRAM_REPORT" -Default "1"
$cliLlmValue = Get-EnvValue -Name "STEER_CLI_LLM" -Default ""
$mailToTarget = Get-EnvValue -Name "STEER_DEFAULT_MAIL_TO" -Default (Get-GitEmail)

$subjectS1 = "Today Plan Brief S1_$timestamp"
$subjectS2 = "Downloads Triage S2_$timestamp"
$subjectS3 = "Calc Result S3_$timestamp"
$subjectS4 = "Productivity Research S4_$timestamp"
$subjectS5 = "Budget Check S5_$timestamp"
$markerS1 = "RUN_SCOPE_S1_$timestamp"
$markerS2 = "RUN_SCOPE_S2_$timestamp"
$markerS3 = "RUN_SCOPE_S3_$timestamp"
$markerS4 = "RUN_SCOPE_S4_$timestamp"
$markerS5 = "RUN_SCOPE_S5_$timestamp"

Write-Host "Starting Complex Scenarios 1-5 execution..."
Write-Host "PLEASE DO NOT TOUCH THE MOUSE/KEYBOARD DURING EXECUTION"
Write-Host ""
Write-Host "STEER_SCENARIO_MODE=$scenarioModeValue"
Write-Host "STEER_NODE_CAPTURE=1, STEER_NODE_CAPTURE_ALL=$nodeCaptureAllValue"
Write-Host "STEER_TEST_MODE=$testModeValue"
Write-Host "STEER_SCENARIO_IDS=$($selectedScenarioIds -join ',')"
Write-Host "STEER_FAIL_ON_FALLBACK=$failOnFallbackValue"
if (-not [string]::IsNullOrWhiteSpace($cliLlmValue)) {
  Write-Host "STEER_CLI_LLM=$cliLlmValue"
} else {
  Write-Host "STEER_CLI_LLM=disabled (default OpenAI path)"
}
Write-Host ""

$runnerPath = Join-Path $PSScriptRoot "run_nl_request_with_telegram.ps1"
$successCount = 0
$failCount = 0

$scenarios = @(
  [PSCustomObject]@{
    Id = 1
    Name = "Calendar -> Notes -> Notepad -> Mail"
    Goal = "Calendar note handoff chain"
    Request = "Open Calendar. Then open Notes and create a note titled '$subjectS1'. Write exactly these lines: 'Calendar opened', 'Notes draft ready', 'Mail prep pending', '$markerS1'. Copy all text. Open Notepad and paste it. Add one line 'Shared via Notepad'. Copy all again. Open Outlook and create a new mail to '$mailToTarget' with subject '$subjectS1', paste body, and send."
  }
  [PSCustomObject]@{
    Id = 2
    Name = "Explorer(Downloads) -> Notes -> Notepad -> Mail"
    Goal = "Downloads triage chain"
    Request = "Open File Explorer and focus Downloads. Open Notes and create note '$subjectS2'. Add lines: 'invoice.pdf', 'screenshot.png', 'notes.txt', '$markerS2'. Copy all text. Open Notepad, paste, add 'Shared via Notes'. Copy all. Open Outlook, compose new mail to '$mailToTarget' with subject '$subjectS2', paste content, and send."
  }
  [PSCustomObject]@{
    Id = 3
    Name = "Calculator -> Notes -> Notepad -> Mail"
    Goal = "Calculation and mail chain"
    Request = "Open Calculator and evaluate 120*1300. Open Notes and create note '$subjectS3'. Add lines: '120*1300=', 'Done', '$markerS3'. Copy all. Open Notepad, paste, add line 'Calc verified'. Copy all. Open Outlook, draft to '$mailToTarget' with subject '$subjectS3', paste content, and send."
  }
  [PSCustomObject]@{
    Id = 4
    Name = "Calendar -> Notes -> Notepad -> Mail"
    Goal = "Research shortlist chain"
    Request = "Open Calendar. Open Notes and create note '$subjectS4'. Add lines: 'focus music', 'pomodoro timer', 'daily review template', '$markerS4'. Copy all. Open Notepad and paste. Add line 'Research shortlist ready'. Copy all. Open Outlook, send to '$mailToTarget' with subject '$subjectS4' and pasted body."
  }
  [PSCustomObject]@{
    Id = 5
    Name = "Explorer(Desktop) -> Calculator -> Notes -> Notepad -> Mail"
    Goal = "Budget draft chain"
    Request = "Open File Explorer to Desktop. Open Calculator and evaluate 120*1450. Open Notes and create note '$subjectS5'. Add lines: 'Base: 120 USD', '120*1450=', '$markerS5'. Copy all. Open Notepad and paste, add line 'Budget draft ready'. Copy all. Open Outlook, create mail to '$mailToTarget' with subject '$subjectS5', paste body, send."
  }
)

foreach ($scenario in $scenarios) {
  if ($selectedScenarioIds -contains ([string]$scenario.Id)) {
    $ok = Invoke-Scenario `
      -ScenarioId $scenario.Id `
      -ScenarioName $scenario.Name `
      -ScenarioGoal $scenario.Goal `
      -ScenarioRequest $scenario.Request `
      -Timestamp $timestamp `
      -RequireTelegramValue $requireTelegramReportValue `
      -RunnerPath $runnerPath
    if ($ok) {
      $successCount++
    } else {
      $failCount++
    }
    Start-Sleep -Seconds 2
  } else {
    Write-Host ("Scenario {0} skipped (STEER_SCENARIO_IDS={1})" -f $scenario.Id, ($selectedScenarioIds -join ","))
  }
}

$summaryFile = Join-Path "scenario_results" ("complex_scenarios_{0}.summary.txt" -f $timestamp)
@(
  "timestamp=$timestamp"
  "selected=$selectedScenarioCount"
  "success=$successCount"
  "failed=$failCount"
  ("status={0}" -f $(if ($failCount -gt 0) { "failed" } else { "success" }))
) | Set-Content -Path $summaryFile -Encoding UTF8

Write-Host ""
Write-Host ("Summary: selected={0}, success={1}, failed={2}" -f $selectedScenarioCount, $successCount, $failCount)
Write-Host ("summary file: {0}" -f $summaryFile)
if ($failCount -gt 0) {
  Write-Host "Completed with failures."
  exit 1
}
Write-Host "All selected complex scenarios succeeded."
exit 0
