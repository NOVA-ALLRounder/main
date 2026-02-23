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

function Get-ObjectValue {
  param(
    [Parameter(Mandatory = $false)][object]$InputObject,
    [Parameter(Mandatory = $true)][string[]]$CandidateNames
  )
  if ($null -eq $InputObject) {
    return $null
  }
  foreach ($name in $CandidateNames) {
    $prop = $InputObject.PSObject.Properties[$name]
    if ($null -ne $prop) {
      return $prop.Value
    }
  }
  return $null
}

function Get-BoolString {
  param([Parameter(Mandatory = $false)][object]$Value)
  if ($null -eq $Value) {
    return "false"
  }
  if ($Value -is [bool]) {
    if ($Value) { return "true" }
    return "false"
  }
  $text = ([string]$Value).Trim().ToLowerInvariant()
  if ($text -in @("1", "true", "yes", "on")) {
    return "true"
  }
  return "false"
}

function Invoke-ApiJsonSafe {
  param(
    [Parameter(Mandatory = $true)][ValidateSet("Get", "Post")] [string]$Method,
    [Parameter(Mandatory = $true)][string]$Uri,
    [Parameter(Mandatory = $false)][object]$Body,
    [Parameter(Mandatory = $false)][int]$TimeoutSec = 30
  )
  try {
    if ($Method -eq "Get") {
      return Invoke-RestMethod -Method Get -Uri $Uri -TimeoutSec $TimeoutSec
    }
    $jsonBody = if ($null -eq $Body) { "{}" } else { $Body | ConvertTo-Json -Depth 12 -Compress }
    return Invoke-RestMethod -Method Post -Uri $Uri -ContentType "application/json" -Body $jsonBody -TimeoutSec $TimeoutSec
  } catch {
    return $null
  }
}

function Test-ApiHealthy {
  param(
    [Parameter(Mandatory = $true)][string]$Uri,
    [Parameter(Mandatory = $false)][int]$TimeoutSec = 3
  )
  try {
    $null = Invoke-WebRequest -Method Get -Uri $Uri -UseBasicParsing -TimeoutSec $TimeoutSec
    return $true
  } catch {
    return $false
  }
}

function Wait-ApiHealthy {
  param(
    [Parameter(Mandatory = $true)][string]$Uri,
    [Parameter(Mandatory = $false)][int]$MaxWaitSec = 60
  )
  for ($i = 0; $i -lt $MaxWaitSec; $i++) {
    if (Test-ApiHealthy -Uri $Uri -TimeoutSec 2) {
      return $true
    }
    Start-Sleep -Seconds 1
  }
  return $false
}

function Get-Score {
  param(
    [Parameter(Mandatory = $true)][string]$Status,
    [Parameter(Mandatory = $true)][string]$PlannerComplete,
    [Parameter(Mandatory = $true)][string]$ExecutionComplete,
    [Parameter(Mandatory = $true)][string]$BusinessComplete,
    [Parameter(Mandatory = $true)][int]$FailedAssertions
  )
  $score = 0
  if ($PlannerComplete -eq "true") { $score += 20 }
  if ($ExecutionComplete -eq "true") { $score += 25 }
  if ($BusinessComplete -eq "true") { $score += 35 }
  $statusLower = $Status.Trim().ToLowerInvariant()
  if ($statusLower -in @("completed", "success")) { $score += 10 }
  if ($FailedAssertions -eq 0) {
    $score += 10
  } else {
    $score -= [Math]::Min(10, $FailedAssertions * 2)
  }
  if ($score -lt 0) { $score = 0 }
  if ($score -gt 100) { $score = 100 }
  return $score
}

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$outDir = Join-Path "scenario_results" "profile_matrix_$timestamp"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$outDirAbs = (Resolve-Path $outDir).Path

$apiPort = Get-EnvValue -Name "STEER_PROFILE_MATRIX_PORT" -Default "5682"
$apiBase = Get-EnvValue -Name "STEER_API_BASE" -Default ("http://127.0.0.1:{0}/api" -f $apiPort)
$healthUrl = Get-EnvValue -Name "STEER_API_HEALTH_URL" -Default ("{0}/system/health" -f $apiBase)
$prompt = Get-EnvValue -Name "STEER_PROFILE_MATRIX_PROMPT" -Default "Find one flight ticket for March 10, 2026 and summarize options."
$profilesRaw = Get-EnvValue -Name "STEER_PROFILE_MATRIX_PROFILES" -Default "fast"
$execTimeoutSec = Get-IntValue -Value (Get-EnvValue -Name "STEER_PROFILE_MATRIX_EXEC_TIMEOUT_SEC" -Default "120") -Default 120
$minScore = Get-IntValue -Value (Get-EnvValue -Name "STEER_PROFILE_MATRIX_MIN_SCORE" -Default "35") -Default 35
$softFail = Get-EnvValue -Name "STEER_PROFILE_MATRIX_SOFT_FAIL" -Default "0"
$assumeApproved = Get-EnvValue -Name "STEER_PROFILE_MATRIX_ASSUME_APPROVED" -Default "1"
$approvalDecision = Get-EnvValue -Name "STEER_PROFILE_MATRIX_APPROVAL_DECISION" -Default "allow_once"
$maxApprovalLoops = Get-IntValue -Value (Get-EnvValue -Name "STEER_PROFILE_MATRIX_MAX_APPROVAL_LOOPS" -Default "8") -Default 8
$requireBusiness = Get-EnvValue -Name "STEER_PROFILE_MATRIX_REQUIRE_BUSINESS" -Default "1"

$csvFile = Join-Path $outDirAbs "matrix.csv"
$mdFile = Join-Path $outDirAbs "matrix.md"
$summaryFile = Join-Path $outDirAbs "summary.txt"
$coreLog = Join-Path $outDirAbs "core_server.log"
$coreErrLog = Join-Path $outDirAbs "core_server.stderr.log"
$corePidFile = Join-Path $outDirAbs "core.pid"

$coreStarted = $false
$coreProcess = $null
$failed = 0
$total = 0
$exitCode = 0

try {
  if (-not (Test-ApiHealthy -Uri $healthUrl -TimeoutSec 2)) {
    Write-Host "API not running. Starting core server..."
    $coreDir = Join-Path $PSScriptRoot "core"
    $bootstrap = @"
`$env:STEER_API_PORT = '$apiPort'
`$env:STEER_API_ALLOW_NO_KEY = '1'
`$env:STEER_DEV_LOCAL_MODE = '1'
`$env:STEER_API_KEY = ''
cargo run --bin local_os_agent
"@
    $coreProcess = Start-Process `
      -FilePath "powershell.exe" `
      -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $bootstrap) `
      -WorkingDirectory $coreDir `
      -RedirectStandardOutput $coreLog `
      -RedirectStandardError $coreErrLog `
      -PassThru
    Set-Content -Path $corePidFile -Value $coreProcess.Id -Encoding UTF8
    $coreStarted = $true

    if (-not (Wait-ApiHealthy -Uri $healthUrl -MaxWaitSec 60)) {
      throw "Failed to start API server. See $coreLog and $coreErrLog"
    }
  }

  "profile,status,run_id,planner_complete,execution_complete,business_complete,business_gate_pass,failed_assertions,approval_loops,score" | Set-Content -Path $csvFile -Encoding UTF8
  @(
    "# Profile Matrix Regression"
    ""
    "- timestamp: $timestamp"
    "- api: $apiBase"
    "- prompt: $prompt"
    "- assume_approved: $assumeApproved"
    "- approval_decision: $approvalDecision"
    "- max_approval_loops: $maxApprovalLoops"
    "- require_business: $requireBusiness"
    ""
  ) | Set-Content -Path $mdFile -Encoding UTF8

  $profileList = @()
  foreach ($token in ($profilesRaw -split ",")) {
    $candidate = $token.Trim().ToLowerInvariant()
    if ($candidate -in @("strict", "test", "fast")) {
      if (-not $profileList.Contains($candidate)) {
        $profileList += $candidate
      }
    } elseif (-not [string]::IsNullOrWhiteSpace($candidate)) {
      Write-Host "Skip unknown profile: $candidate"
    }
  }
  if ($profileList.Count -eq 0) {
    throw "No valid profiles selected. Allowed values: strict,test,fast"
  }

  foreach ($profile in $profileList) {
    $total++
    Write-Host "Running profile: $profile"

    $intentResp = Invoke-ApiJsonSafe -Method Post -Uri ("{0}/agent/intent" -f $apiBase) -Body @{ text = $prompt } -TimeoutSec 30
    if ($null -eq $intentResp) {
      Write-Host "  intent failed"
      $failed++
      Add-Content -Path $csvFile -Value ("{0},error,,false,false,false,0,0,0,0" -f $profile) -Encoding UTF8
      continue
    }
    $sessionId = [string](Get-ObjectValue -InputObject $intentResp -CandidateNames @("session_id", "sessionId"))
    if ([string]::IsNullOrWhiteSpace($sessionId)) {
      Write-Host "  missing session_id"
      $failed++
      Add-Content -Path $csvFile -Value ("{0},error,,false,false,false,0,0,0,0" -f $profile) -Encoding UTF8
      continue
    }

    $planResp = Invoke-ApiJsonSafe -Method Post -Uri ("{0}/agent/plan" -f $apiBase) -Body @{ session_id = $sessionId } -TimeoutSec 30
    if ($null -eq $planResp) {
      Write-Host "  plan failed"
      $failed++
      Add-Content -Path $csvFile -Value ("{0},error,,false,false,false,0,0,0,0" -f $profile) -Encoding UTF8
      continue
    }
    $planId = [string](Get-ObjectValue -InputObject $planResp -CandidateNames @("plan_id", "planId"))
    if ([string]::IsNullOrWhiteSpace($planId)) {
      Write-Host "  missing plan_id"
      $failed++
      Add-Content -Path $csvFile -Value ("{0},error,,false,false,false,0,0,0,0" -f $profile) -Encoding UTF8
      continue
    }

    $execPayload = @{ plan_id = $planId; profile = $profile }
    $execResp = Invoke-ApiJsonSafe -Method Post -Uri ("{0}/agent/execute" -f $apiBase) -Body $execPayload -TimeoutSec $execTimeoutSec
    if ($null -eq $execResp) {
      Write-Host "  execute timeout/error"
      $failed++
      Add-Content -Path $csvFile -Value ("{0},error,,false,false,false,0,0,0,0" -f $profile) -Encoding UTF8
      continue
    }

    $status = [string](Get-ObjectValue -InputObject $execResp -CandidateNames @("status"))
    $approvalLoops = 0

    while ($assumeApproved -eq "1" -and $status -eq "approval_required" -and $approvalLoops -lt $maxApprovalLoops) {
      $approval = Get-ObjectValue -InputObject $execResp -CandidateNames @("approval")
      $approvalAction = [string](Get-ObjectValue -InputObject $approval -CandidateNames @("action"))
      if ([string]::IsNullOrWhiteSpace($approvalAction)) {
        break
      }

      $approveResp = Invoke-ApiJsonSafe `
        -Method Post `
        -Uri ("{0}/agent/approve" -f $apiBase) `
        -Body @{ plan_id = $planId; action = $approvalAction; decision = $approvalDecision } `
        -TimeoutSec 30
      if ($null -eq $approveResp) {
        $status = "error"
        break
      }

      $approvalStatus = [string](Get-ObjectValue -InputObject $approveResp -CandidateNames @("status"))
      if ($approvalStatus -eq "denied") {
        $status = "denied"
        break
      }

      $approvalLoops++
      $execResp = Invoke-ApiJsonSafe -Method Post -Uri ("{0}/agent/execute" -f $apiBase) -Body $execPayload -TimeoutSec $execTimeoutSec
      if ($null -eq $execResp) {
        $status = "error"
        break
      }
      $status = [string](Get-ObjectValue -InputObject $execResp -CandidateNames @("status"))
    }

    $runId = [string](Get-ObjectValue -InputObject $execResp -CandidateNames @("run_id", "runId"))
    $plannerComplete = Get-BoolString -Value (Get-ObjectValue -InputObject $execResp -CandidateNames @("planner_complete", "plannerComplete"))
    $executionComplete = Get-BoolString -Value (Get-ObjectValue -InputObject $execResp -CandidateNames @("execution_complete", "executionComplete"))
    $businessComplete = Get-BoolString -Value (Get-ObjectValue -InputObject $execResp -CandidateNames @("business_complete", "businessComplete"))

    $failedAssertions = 0
    if (-not [string]::IsNullOrWhiteSpace($runId)) {
      $assertionsResp = Invoke-ApiJsonSafe -Method Get -Uri ("{0}/agent/task-runs/{1}/assertions" -f $apiBase, $runId) -TimeoutSec 30
      if ($null -ne $assertionsResp) {
        foreach ($item in @($assertionsResp)) {
          $passed = Get-ObjectValue -InputObject $item -CandidateNames @("passed")
          if (-not [bool]$passed) {
            $failedAssertions++
          }
        }
      }
    }

    $score = Get-Score `
      -Status $status `
      -PlannerComplete $plannerComplete `
      -ExecutionComplete $executionComplete `
      -BusinessComplete $businessComplete `
      -FailedAssertions $failedAssertions

    $businessGatePass = 1
    if ($requireBusiness -eq "1" -and $businessComplete -ne "true") {
      $businessGatePass = 0
    }

    Add-Content -Path $csvFile -Value (
      "{0},{1},{2},{3},{4},{5},{6},{7},{8},{9}" -f
      $profile,
      $status,
      $runId,
      $plannerComplete,
      $executionComplete,
      $businessComplete,
      $businessGatePass,
      $failedAssertions,
      $approvalLoops,
      $score
    ) -Encoding UTF8

    Add-Content -Path $mdFile -Value @(
      "## $profile"
      "- status: $status"
      "- run_id: $(if ([string]::IsNullOrWhiteSpace($runId)) { "n/a" } else { $runId })"
      "- planner_complete: $plannerComplete"
      "- execution_complete: $executionComplete"
      "- business_complete: $businessComplete"
      "- business_gate_pass: $businessGatePass"
      "- failed_assertions: $failedAssertions"
      "- approval_loops: $approvalLoops"
      "- score: $score"
      ""
    ) -Encoding UTF8

    if ($score -lt $minScore -or $businessGatePass -eq 0) {
      $failed++
    }
  }

  $statusValue = if ($failed -eq 0) { "success" } else { "failed" }
  @(
    "timestamp=$timestamp"
    "profiles=$profilesRaw"
    "total=$total"
    "fail=$failed"
    "min_score=$minScore"
    "require_business=$requireBusiness"
    "status=$statusValue"
  ) | Set-Content -Path $summaryFile -Encoding UTF8

  Write-Host ""
  Write-Host "profile matrix summary"
  Get-Content -Path $summaryFile | ForEach-Object { Write-Host "  $_" }
  Write-Host "output: $outDirAbs"

  if ($failed -ne 0 -and $softFail -ne "1") {
    $exitCode = 1
  }
} finally {
  if ($coreStarted -and $null -ne $coreProcess) {
    try {
      if (-not $coreProcess.HasExited) {
        Stop-Process -Id $coreProcess.Id -Force -ErrorAction SilentlyContinue
      }
    } catch {
      # no-op
    }
  }
}

if ($exitCode -ne 0 -and $softFail -eq "1") {
  Write-Host "profile matrix completed with failures (soft-fail enabled)"
  $exitCode = 0
}

exit $exitCode
