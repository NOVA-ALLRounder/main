param(
  [Parameter(Position = 0)][string]$RequestText,
  [Parameter(Position = 1)][string]$TaskName = "Natural language request execution"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location $PSScriptRoot

if ([string]::IsNullOrWhiteSpace($RequestText)) {
  Write-Host 'Usage: .\run_nl_request_with_telegram.ps1 "<request text>" ["task name"]'
  exit 1
}

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

function Get-IntValue {
  param(
    [Parameter(Mandatory = $true)][string]$Value,
    [Parameter(Mandatory = $true)][int]$Default
  )
  $parsed = 0
  if ([int]::TryParse($Value, [ref]$parsed)) {
    return $parsed
  }
  return $Default
}

function Is-Truthy {
  param([Parameter(Mandatory = $false)][string]$Value)
  if ($null -eq $Value) { return $false }
  return ($Value.Trim().ToLowerInvariant() -in @("1", "true", "yes", "on"))
}

function Load-DotEnvFile {
  param([Parameter(Mandatory = $true)][string]$Path)
  if (-not (Test-Path $Path)) {
    return
  }
  foreach ($line in Get-Content -Path $Path) {
    $trimmed = $line.Trim()
    if ($trimmed.Length -eq 0 -or $trimmed.StartsWith("#")) {
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
    if ($name.Length -gt 0) {
      [Environment]::SetEnvironmentVariable($name, $value)
    }
  }
}

function Detect-CliLlmProvider {
  $preferred = Get-EnvValue -Name "STEER_CLI_LLM_AUTO_ORDER" -Default "codex,gemini,claude"
  foreach ($token in ($preferred -split ",")) {
    $candidate = $token.Trim().ToLowerInvariant()
    if ([string]::IsNullOrWhiteSpace($candidate)) {
      continue
    }
    if ($null -ne (Get-Command $candidate -ErrorAction SilentlyContinue)) {
      return $candidate
    }
  }
  return ""
}

function Has-OpenAiKeyConfigured {
  if (-not [string]::IsNullOrWhiteSpace($env:OPENAI_API_KEY)) {
    return $true
  }
  foreach ($candidate in @(".env", "core/.env")) {
    if (-not (Test-Path $candidate)) {
      continue
    }
    $line = Select-String -Path $candidate -Pattern '^\s*OPENAI_API_KEY\s*=' | Select-Object -Last 1
    if ($null -eq $line) {
      continue
    }
    $raw = $line.Line -replace '^\s*OPENAI_API_KEY\s*=\s*', ''
    $raw = $raw.Trim().Trim('"').Trim("'")
    if (-not [string]::IsNullOrWhiteSpace($raw)) {
      return $true
    }
  }
  return $false
}

function Request-RequiresMailSend {
  param([Parameter(Mandatory = $true)][string]$Text)
  return ($Text -match '(mail|email|gmail|outlook|이메일|메일|보내)')
}

function Get-MailProof {
  param([Parameter(Mandatory = $true)][string]$LogPath)

  $default = [PSCustomObject]@{
    Status = "none"
    Recipient = ""
    Subject = ""
    BodyLen = "-1"
  }
  if (-not (Test-Path $LogPath)) {
    return $default
  }

  $line = Get-Content -Path $LogPath | Where-Object { $_ -match 'MAIL_SEND_PROOF\|status=' } | Select-Object -Last 1
  if ([string]::IsNullOrWhiteSpace($line)) {
    return $default
  }

  $status = ""
  $recipient = ""
  $subject = ""
  $bodyLen = "-1"

  $m = [regex]::Match($line, 'status=([^|]*)')
  if ($m.Success) { $status = $m.Groups[1].Value }
  $m = [regex]::Match($line, 'recipient=([^|]*)')
  if ($m.Success) { $recipient = $m.Groups[1].Value }
  $m = [regex]::Match($line, 'subject=([^|]*)')
  if ($m.Success) { $subject = $m.Groups[1].Value }
  $m = [regex]::Match($line, 'body_len=([^|]*)')
  if ($m.Success) { $bodyLen = $m.Groups[1].Value }

  if ([string]::IsNullOrWhiteSpace($status)) { $status = "none" }

  return [PSCustomObject]@{
    Status = $status
    Recipient = $recipient
    Subject = $subject
    BodyLen = $bodyLen
  }
}

function Get-TerminalBlockStatus {
  param([Parameter(Mandatory = $true)][string]$LogPath)
  if (-not (Test-Path $LogPath)) {
    return ""
  }
  foreach ($statusName in @("blocked", "approval_required", "manual_required")) {
    $pattern = "RUN_ATTEMPT\|phase=execution_end\|status=$statusName(\||$)"
    if (Select-String -Path $LogPath -Pattern $pattern -Quiet) {
      return $statusName
    }
  }
  return ""
}

function Send-TelegramMessage {
  param(
    [Parameter(Mandatory = $true)][string]$BotToken,
    [Parameter(Mandatory = $true)][string]$ChatId,
    [Parameter(Mandatory = $true)][string]$Text,
    [Parameter(Mandatory = $false)][int]$TimeoutSec = 30
  )

  $uri = "https://api.telegram.org/bot$BotToken/sendMessage"
  $payload = @{
    chat_id = $ChatId
    text = $Text
    disable_web_page_preview = "true"
  }

  try {
    $response = Invoke-RestMethod -Method Post -Uri $uri -Body $payload -TimeoutSec $TimeoutSec
    $ok = $response.PSObject.Properties["ok"]
    if ($null -ne $ok -and [bool]$ok.Value) {
      return $true
    }
    return $false
  } catch {
    return $false
  }
}

Load-DotEnvFile -Path "core/.env"

$ts = Get-Date -Format "yyyyMMdd_HHmmss"
New-Item -ItemType Directory -Force -Path "scenario_results" | Out-Null

$logFile = Join-Path "scenario_results" "nl_request_$ts.log"
$rawMsgFile = Join-Path "scenario_results" "nl_request_$ts.telegram.raw.txt"
$finalMsgFile = Join-Path "scenario_results" "nl_request_$ts.telegram.final.txt"
$nodeDir = Join-Path "scenario_results" "nl_request_${ts}_nodes"

$scenarioModeValue = Get-EnvValue -Name "STEER_SCENARIO_MODE" -Default "0"
$nodeCaptureAllValue = Get-EnvValue -Name "STEER_NODE_CAPTURE_ALL" -Default "1"
$cliLlmValue = Get-EnvValue -Name "STEER_CLI_LLM" -Default ""
$failOnFallbackValue = Get-EnvValue -Name "STEER_FAIL_ON_FALLBACK" -Default "1"
$requirePrimaryPlannerValue = Get-EnvValue -Name "STEER_REQUIRE_PRIMARY_PLANNER" -Default "1"
$lockDisabledValue = Get-EnvValue -Name "STEER_LOCK_DISABLED" -Default "0"
$approvalAskFallbackValue = Get-EnvValue -Name "STEER_APPROVAL_ASK_FALLBACK" -Default "deny"
$runScopeEnabled = Get-EnvValue -Name "STEER_SEMANTIC_RUN_SCOPE" -Default "1"
$requireTelegramReportValue = Get-EnvValue -Name "STEER_REQUIRE_TELEGRAM_REPORT" -Default "1"
$testModeValue = Get-EnvValue -Name "STEER_TEST_MODE" -Default "0"
$deterministicAutoplanValue = Get-EnvValue -Name "STEER_DETERMINISTIC_GOAL_AUTOPLAN" -Default "1"
$openAiPreflightRequiredValue = Get-EnvValue -Name "STEER_PREFLIGHT_REQUIRE_OPENAI_KEY" -Default "0"
$requireMailBodyValue = Get-EnvValue -Name "STEER_REQUIRE_MAIL_BODY" -Default "1"
$requireMailSubjectValue = Get-EnvValue -Name "STEER_REQUIRE_MAIL_SUBJECT" -Default "1"
$requireSentMailboxEvidenceValue = Get-EnvValue -Name "STEER_REQUIRE_SENT_MAILBOX_EVIDENCE" -Default "1"
$maxNewItemActionsValue = Get-IntValue -Value (Get-EnvValue -Name "STEER_MAX_NEW_ITEM_ACTIONS" -Default "6") -Default 6

if ([string]::IsNullOrWhiteSpace($cliLlmValue) -and (Get-EnvValue -Name "STEER_AUTO_DETECT_CLI_LLM" -Default "1") -eq "1") {
  $detected = Detect-CliLlmProvider
  if (-not [string]::IsNullOrWhiteSpace($detected)) {
    $cliLlmValue = $detected
    Write-Host "Auto-detected CLI LLM provider: $cliLlmValue"
  }
}

if ($requirePrimaryPlannerValue -eq "1" -and $scenarioModeValue -eq "1" -and (Get-EnvValue -Name "STEER_ALLOW_SCENARIO_MODE" -Default "0") -ne "1") {
  Write-Error "Policy violation: STEER_SCENARIO_MODE=1 requires STEER_ALLOW_SCENARIO_MODE=1"
  exit 1
}

if (Is-Truthy $lockDisabledValue) {
  $allowLockDisabled = Is-Truthy (Get-EnvValue -Name "STEER_ALLOW_LOCK_DISABLED_NON_TEST" -Default "0")
  if (-not (Is-Truthy $testModeValue) -and -not (Is-Truthy (Get-EnvValue -Name "CI" -Default "0")) -and -not $allowLockDisabled) {
    Write-Error "Policy violation: STEER_LOCK_DISABLED=1 is only allowed in TEST/CI by default"
    exit 1
  }
}

if ($openAiPreflightRequiredValue -eq "1" -and $scenarioModeValue -eq "0" -and [string]::IsNullOrWhiteSpace($cliLlmValue) -and -not (Has-OpenAiKeyConfigured)) {
  Write-Error "Preflight failed: OPENAI_API_KEY is not set"
  exit 1
}

$requestTextExec = $RequestText
$runScopeMarker = ""
if ($runScopeEnabled -eq "1") {
  $runScopeMarker = "RUN_SCOPE_$ts"
  $requestTextExec = "$RequestText`n`nFinal line must contain marker: $runScopeMarker"
}

Write-Host "Running NL request..."
Write-Host "Task: $TaskName"
Write-Host "Mode: STEER_SCENARIO_MODE=$scenarioModeValue"
Write-Host "Node Capture: STEER_NODE_CAPTURE=1, STEER_NODE_CAPTURE_ALL=$nodeCaptureAllValue"
Write-Host "Test Mode: STEER_TEST_MODE=$testModeValue"
Write-Host "Mail Body Required: STEER_REQUIRE_MAIL_BODY=$requireMailBodyValue"
Write-Host "Mail Subject Required: STEER_REQUIRE_MAIL_SUBJECT=$requireMailSubjectValue"
Write-Host "Sent Mailbox Evidence Required: STEER_REQUIRE_SENT_MAILBOX_EVIDENCE=$requireSentMailboxEvidenceValue"
Write-Host "Fallback Policy: STEER_FAIL_ON_FALLBACK=$failOnFallbackValue"
if (-not [string]::IsNullOrWhiteSpace($runScopeMarker)) {
  Write-Host "Semantic Scope Marker: $runScopeMarker"
}
if (-not [string]::IsNullOrWhiteSpace($cliLlmValue)) {
  Write-Host "CLI LLM: STEER_CLI_LLM=$cliLlmValue"
} else {
  Write-Host "CLI LLM: disabled (default OpenAI path)"
}

$envOverrides = @{
  "STEER_SCENARIO_MODE" = $scenarioModeValue
  "STEER_NODE_CAPTURE" = "1"
  "STEER_NODE_CAPTURE_ALL" = $nodeCaptureAllValue
  "STEER_NODE_CAPTURE_DIR" = $nodeDir
  "STEER_LOCK_DISABLED" = $lockDisabledValue
  "STEER_APPROVAL_ASK_FALLBACK" = $approvalAskFallbackValue
  "STEER_TEST_MODE" = $testModeValue
  "STEER_DETERMINISTIC_GOAL_AUTOPLAN" = $deterministicAutoplanValue
}
if (-not [string]::IsNullOrWhiteSpace($cliLlmValue)) {
  $envOverrides["STEER_CLI_LLM"] = $cliLlmValue
}

$backupEnv = @{}
foreach ($entry in $envOverrides.GetEnumerator()) {
  $backupEnv[$entry.Key] = [Environment]::GetEnvironmentVariable($entry.Key)
  [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value)
}

$agentExitCode = 1
try {
  & cargo run --manifest-path core/Cargo.toml --bin local_os_agent -- surf $requestTextExec *> $logFile
  $agentExitCode = $LASTEXITCODE
} finally {
  foreach ($entry in $backupEnv.GetEnumerator()) {
    [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value)
  }
}

$status = if ($agentExitCode -eq 0) { "success" } else { "failed" }

$hardFatalPattern = 'Failed to acquire lock|thread .* panicked|FATAL ERROR|LLM not available for surf mode|Preflight failed|Surf failed|Execution Error|SCHEMA_ERROR'
$softFatalPattern = 'Supervisor escalated|PLAN_REJECTED|LLM Refused'
$fatalPattern = $hardFatalPattern
if (Is-Truthy (Get-EnvValue -Name "STEER_FATAL_STRICT" -Default "0")) {
  $fatalPattern = "$hardFatalPattern|$softFatalPattern"
}
if (Test-Path $logFile) {
  if (Select-String -Path $logFile -Pattern $fatalPattern -Quiet) {
    $status = "failed"
  }
}

$terminalBlockStatus = Get-TerminalBlockStatus -LogPath $logFile
if (-not [string]::IsNullOrWhiteSpace($terminalBlockStatus)) {
  $status = "failed"
}

$fallbackHit = 0
if (Test-Path $logFile -and (Select-String -Path $logFile -Pattern 'fallback action|FALLBACK_ACTION:' -Quiet)) {
  $fallbackHit = 1
  if ($failOnFallbackValue -eq "1") {
    $status = "failed"
  }
}

$newItemActionCount = 0
if (Test-Path $logFile) {
  $newItemActionCount = (Select-String -Path $logFile -Pattern "Shortcut 'n'.*Created new item|mail_draft_ready|shortcut cmd\+n").Count
  if ($newItemActionCount -gt $maxNewItemActionsValue) {
    $status = "failed"
  }
}

$mailProof = Get-MailProof -LogPath $logFile
$requestRequiresMail = Request-RequiresMailSend -Text $RequestText
if ($requestRequiresMail -or (Get-EnvValue -Name "STEER_REQUIRE_MAIL_SEND" -Default "0") -eq "1") {
  if ($mailProof.Status -ne "sent_confirmed") {
    $status = "failed"
  }
}
if ($requireMailSubjectValue -eq "1" -and $mailProof.Status -eq "sent_confirmed" -and [string]::IsNullOrWhiteSpace($mailProof.Subject)) {
  $status = "failed"
}
if ($requireMailBodyValue -eq "1" -and $mailProof.Status -eq "sent_confirmed") {
  $bodyLen = Get-IntValue -Value $mailProof.BodyLen -Default -1
  if ($bodyLen -lt 1) {
    $status = "failed"
  }
}
if ($requireSentMailboxEvidenceValue -eq "1" -and $requestRequiresMail -and $mailProof.Status -ne "sent_confirmed") {
  $status = "failed"
}

$mailProofLine = "MAIL_SEND_PROOF|status={0}|recipient={1}|subject={2}|body_len={3}|" -f $mailProof.Status, $mailProof.Recipient, $mailProof.Subject, $mailProof.BodyLen
Write-Output $mailProofLine

$finalExecutionStatus = if ($status -eq "success") { "success" } else { "failed" }
$executionAttemptLine = "RUN_ATTEMPT|phase=execution_end|status={0}|details=terminal_status={1},fallback={2},new_item_count={3},mail_proof={4}|ts={5}" -f `
  $finalExecutionStatus, `
  $(if ([string]::IsNullOrWhiteSpace($terminalBlockStatus)) { "none" } else { $terminalBlockStatus }), `
  $fallbackHit, `
  $newItemActionCount, `
  $mailProof.Status, `
  (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
Write-Output $executionAttemptLine

$summaryLine = "Result: {0}; mail_proof={1}; fallback={2}; terminal_status={3}" -f `
  $status, `
  $mailProof.Status, `
  $fallbackHit, `
  $(if ([string]::IsNullOrWhiteSpace($terminalBlockStatus)) { "none" } else { $terminalBlockStatus })

$telegramMessage = @"
[STEER] $TaskName
status: $status
request: $RequestText
$summaryLine
log: $logFile
"@.Trim()

$telegramMessage | Set-Content -Path $rawMsgFile -Encoding UTF8
$telegramMessage | Set-Content -Path $finalMsgFile -Encoding UTF8

$botToken = Get-EnvValue -Name "TELEGRAM_BOT_TOKEN" -Default ""
$chatId = Get-EnvValue -Name "TELEGRAM_CHAT_ID" -Default ""
$telegramTimeout = Get-IntValue -Value (Get-EnvValue -Name "STEER_NOTIFIER_TIMEOUT_SEC" -Default "120") -Default 120

$telegramOk = $false
if (-not [string]::IsNullOrWhiteSpace($botToken) -and -not [string]::IsNullOrWhiteSpace($chatId)) {
  $telegramOk = Send-TelegramMessage -BotToken $botToken -ChatId $chatId -Text $telegramMessage -TimeoutSec $telegramTimeout
  if (-not $telegramOk) {
    Add-Content -Path $finalMsgFile -Value "`n[telegram] send failed" -Encoding UTF8
    $status = "failed"
  }
} else {
  if ($requireTelegramReportValue -eq "1") {
    Add-Content -Path $finalMsgFile -Value "`n[telegram] required but TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID missing" -Encoding UTF8
    Write-Error "Telegram report is required but TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID is missing."
    $status = "failed"
  } else {
    Write-Host "Telegram env missing. Skipped send."
  }
}

Write-Host ""
Write-Host "Done."
Write-Host " - status: $status"
Write-Host " - log: $logFile"
Write-Host " - telegram raw: $rawMsgFile"
Write-Host " - telegram final: $finalMsgFile"

if ($status -eq "success") {
  exit 0
}
exit 1
