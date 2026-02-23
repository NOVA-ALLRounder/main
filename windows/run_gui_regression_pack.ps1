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

function Evaluate-IterationLog {
  param(
    [Parameter(Mandatory = $true)][string]$LogPath,
    [Parameter(Mandatory = $true)][string[]]$SelectedIds,
    [Parameter(Mandatory = $true)][int]$SelectedCount
  )

  $reasons = New-Object System.Collections.Generic.List[string]

  if (-not (Test-Path $LogPath)) {
    [void]$reasons.Add("missing_log_file")
    return [PSCustomObject]@{
      Pass = $false
      Reasons = @($reasons)
    }
  }

  $lines = Get-Content -Path $LogPath

  if (Select-String -Path $LogPath -Pattern 'focus_recovery_failed|cmd_n_loop_guard_block' -Quiet) {
    [void]$reasons.Add("focus_or_cmdn_loop_detected")
  }

  $mailStatusLines = $lines | Where-Object { $_ -match 'MAIL_SEND_PROOF\|status=' }
  $nonConfirmed = $mailStatusLines | Where-Object { $_ -notmatch 'MAIL_SEND_PROOF\|status=sent_confirmed\|' }
  if ($nonConfirmed.Count -gt 0) {
    [void]$reasons.Add("mail_send_non_confirmed_status")
  }

  $sentCount = ($mailStatusLines | Where-Object { $_ -match 'MAIL_SEND_PROOF\|status=sent_confirmed\|' }).Count
  if ($sentCount -lt $SelectedCount) {
    [void]$reasons.Add(("mail_send_proof_count_lt_selected({0}<{1})" -f $sentCount, $SelectedCount))
  }

  foreach ($id in $SelectedIds) {
    $pattern = "RUN_ATTEMPT\|phase=scenario_${id}_final_judgement\|"
    $line = $lines | Where-Object { $_ -match $pattern } | Select-Object -Last 1
    if ([string]::IsNullOrWhiteSpace($line)) {
      [void]$reasons.Add("scenario_${id}_missing_final_judgement")
      continue
    }

    $status = ""
    $details = ""
    $semanticMissing = ""
    $mailProof = ""

    $statusMatch = [regex]::Match($line, '\|status=([^|]+)')
    if ($statusMatch.Success) {
      $status = $statusMatch.Groups[1].Value
    }
    $detailsMatch = [regex]::Match($line, '\|details=(.*)\|ts=')
    if ($detailsMatch.Success) {
      $details = $detailsMatch.Groups[1].Value
    }
    $semanticMatch = [regex]::Match($details, 'semantic_missing=([^,|]+)')
    if ($semanticMatch.Success) {
      $semanticMissing = $semanticMatch.Groups[1].Value
    }
    $mailProofMatch = [regex]::Match($details, 'mail_proof=([^,|]+)')
    if ($mailProofMatch.Success) {
      $mailProof = $mailProofMatch.Groups[1].Value
    }

    if ($status -ne "success") {
      [void]$reasons.Add("scenario_${id}_status_$($status -replace '\s+', '_')")
    }
    if ($semanticMissing -ne "0") {
      if ([string]::IsNullOrWhiteSpace($semanticMissing)) { $semanticMissing = "unknown" }
      [void]$reasons.Add("scenario_${id}_semantic_missing_$semanticMissing")
    }
    if ($mailProof -ne "sent_confirmed") {
      if ([string]::IsNullOrWhiteSpace($mailProof)) { $mailProof = "unknown" }
      [void]$reasons.Add("scenario_${id}_mail_proof_$mailProof")
    }
  }

  return [PSCustomObject]@{
    Pass = ($reasons.Count -eq 0)
    Reasons = @($reasons)
  }
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$outDir = Join-Path "scenario_results" "gui_regression_pack_$timestamp"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$repeatRaw = Get-EnvValue -Name "STEER_GUI_REG_PACK_REPEAT" -Default "1"
$repeat = Get-IntValue -Value $repeatRaw -Default -1
if ($repeat -lt 1) {
  Write-Error "invalid STEER_GUI_REG_PACK_REPEAT=$repeatRaw"
  exit 1
}

$scenariosRaw = Get-EnvValue -Name "STEER_GUI_REG_SCENARIOS" -Default "1,2,3,4,5"
$selectedIds = Normalize-ScenarioIds -Raw $scenariosRaw
if ($selectedIds.Count -eq 0) {
  Write-Error "invalid STEER_GUI_REG_SCENARIOS=$scenariosRaw (allowed: 1,2,3,4,5)"
  exit 1
}

$selectedCount = $selectedIds.Count
$scenariosCsv = ($selectedIds -join ",")

Write-Host "GUI regression pack start"
Write-Host " - repeat: $repeat"
Write-Host " - scenarios: $scenariosCsv"
Write-Host " - output: $outDir"
Write-Host " - mode: approve-assumed + mock external integrations"
Write-Host " - quality gate: final judgement + mail send proof + focus/cmd+n guard"

$passCount = 0
$failCount = 0

for ($i = 1; $i -le $repeat; $i++) {
  $iterLog = Join-Path $outDir ("iteration_{0}.log" -f $i)
  $iterStatus = Join-Path $outDir ("iteration_{0}.status" -f $i)

  Write-Host ""
  Write-Host ("===== iteration {0}/{1} =====" -f $i, $repeat)

  $envOverrides = @{
    "STEER_TEST_MODE" = "1"
    "STEER_TEST_ASSUME_APPROVED" = "1"
    "STEER_N8N_MOCK" = "1"
    "STEER_REQUIRE_TELEGRAM_REPORT" = "0"
    "STEER_NODE_CAPTURE_ALL" = "1"
    "STEER_FAIL_ON_FALLBACK" = "1"
    "STEER_REQUIRE_MAIL_SUBJECT" = "1"
    "STEER_REQUIRE_SENT_MAILBOX_EVIDENCE" = "1"
    "STEER_EXEC_FOCUS_HANDOFF" = "1"
    "STEER_EXEC_FOCUS_HANDOFF_RETRIES" = "3"
    "STEER_EXEC_FOCUS_HANDOFF_FINDER_BRIDGE" = "1"
    "STEER_AX_SNAPSHOT_STRICT" = "1"
    "STEER_PREFLIGHT_AX_SNAPSHOT" = "1"
    "STEER_SEMANTIC_REQUIRE_RUST_CONTRACT" = "1"
    "STEER_SEMANTIC_REQUIRE_NONEMPTY" = "1"
    "STEER_SEMANTIC_FAIL_ON_TRUNCATION" = "1"
    "STEER_INPUT_GUARD_MAX_PAUSES" = "20"
    "STEER_INPUT_GUARD_MAX_PAUSE_SECONDS" = "180"
    "STEER_SCENARIO_IDS" = $scenariosCsv
  }
  $backup = @{}
  foreach ($entry in $envOverrides.GetEnumerator()) {
    $backup[$entry.Key] = [Environment]::GetEnvironmentVariable($entry.Key)
    [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value)
  }

  $exitCode = 1
  try {
    $target = Join-Path $PSScriptRoot "run_complex_scenarios.ps1"
    $outputLines = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $target 2>&1
    $exitCode = $LASTEXITCODE
    if ($null -eq $outputLines) {
      "" | Set-Content -Path $iterLog -Encoding UTF8
    } else {
      $outputLines | Set-Content -Path $iterLog -Encoding UTF8
      $outputLines | ForEach-Object { Write-Host $_ }
    }
  } finally {
    foreach ($entry in $backup.GetEnumerator()) {
      [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value)
    }
  }

  if ($exitCode -eq 0) {
    $quality = Evaluate-IterationLog -LogPath $iterLog -SelectedIds $selectedIds -SelectedCount $selectedCount
    if ($quality.Pass) {
      Write-Host "iteration ${i}: pass"
      $passCount++
      Set-Content -Path $iterStatus -Value ("PASS|{0}|quality=ok" -f $exitCode) -Encoding UTF8
    } else {
      $reasonCsv = ($quality.Reasons -join ",")
      Write-Host "iteration ${i}: fail (quality gate)"
      Write-Host ("  - {0}" -f ($quality.Reasons -join ", "))
      $failCount++
      Set-Content -Path $iterStatus -Value ("FAIL|{0}|quality={1}" -f $exitCode, $reasonCsv) -Encoding UTF8
    }
  } else {
    Write-Host "iteration ${i}: fail (exit=$exitCode)"
    $failCount++
    Set-Content -Path $iterStatus -Value ("FAIL|{0}" -f $exitCode) -Encoding UTF8
  }
}

$summaryFile = Join-Path $outDir "summary.txt"
@(
  "timestamp=$timestamp"
  "repeat=$repeat"
  "scenarios=$scenariosCsv"
  "pass=$passCount"
  "fail=$failCount"
  ("status={0}" -f $(if ($failCount -gt 0) { "failed" } else { "success" }))
) | Set-Content -Path $summaryFile -Encoding UTF8

$junitFile = Join-Path $outDir "junit.xml"
$xml = New-Object System.Collections.Generic.List[string]
[void]$xml.Add('<?xml version="1.0" encoding="UTF-8"?>')
[void]$xml.Add(("<testsuite name=""gui_regression_pack"" tests=""{0}"" failures=""{1}"" timestamp=""{2}"">" -f $repeat, $failCount, (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")))
for ($i = 1; $i -le $repeat; $i++) {
  $iterLog = Join-Path $outDir ("iteration_{0}.log" -f $i)
  $iterStatus = Join-Path $outDir ("iteration_{0}.status" -f $i)
  $statusLine = if (Test-Path $iterStatus) { (Get-Content $iterStatus -Raw).Trim() } else { "FAIL|999" }
  $parts = $statusLine -split '\|', 3
  $statusKind = if ($parts.Count -ge 1) { $parts[0] } else { "FAIL" }
  $exitCode = if ($parts.Count -ge 2) { $parts[1] } else { "999" }
  $statusMeta = if ($parts.Count -ge 3) { $parts[2] } else { "" }

  if ($statusKind -eq "PASS") {
    [void]$xml.Add(("  <testcase name=""iteration_{0}""/>" -f $i))
  } else {
    $message = if ([string]::IsNullOrWhiteSpace($statusMeta)) { "exit=$exitCode" } else { "exit=$exitCode;$statusMeta" }
    $escapedMessage = [System.Security.SecurityElement]::Escape($message)
    $tailText = ""
    if (Test-Path $iterLog) {
      $tailLines = Get-Content -Path $iterLog -Tail 120
      $tailText = [string]::Join("`n", $tailLines)
    }
    $escapedTail = [System.Security.SecurityElement]::Escape($tailText)
    [void]$xml.Add(("  <testcase name=""iteration_{0}"">" -f $i))
    [void]$xml.Add(("    <failure message=""{0}"">{1}</failure>" -f $escapedMessage, $escapedTail))
    [void]$xml.Add("  </testcase>")
  }
}
[void]$xml.Add("</testsuite>")
$xml | Set-Content -Path $junitFile -Encoding UTF8

Write-Host ""
Write-Host "Regression summary"
Write-Host " - pass: $passCount"
Write-Host " - fail: $failCount"
Write-Host " - summary: $summaryFile"
Write-Host " - junit: $junitFile"

if ($failCount -gt 0) {
  exit 1
}
exit 0
