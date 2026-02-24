param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"

Set-Location $PSScriptRoot

$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$outDir = Join-Path "scenario_results" "priority_regression_$timestamp"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$coreLog = Join-Path $outDir "core_tests.log"
$webLog = Join-Path $outDir "web_build.log"
$contractLog = Join-Path $outDir "contract_checks.log"
$profileMatrixLog = Join-Path $outDir "profile_matrix.log"
$summaryFile = Join-Path $outDir "summary.txt"

$passCount = 0
$failCount = 0

function Invoke-Step {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$LogPath,
    [Parameter(Mandatory = $true)][scriptblock]$Action
  )

  Write-Host "== $Name =="
  try {
    & $Action *> $LogPath
    if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
      throw "$Name exited with code $LASTEXITCODE"
    }
    Write-Host "OK $Name"
    $script:passCount++
  } catch {
    Write-Host "FAIL $Name (see $LogPath)"
    Add-Content -Path $LogPath -Value "`n[ERROR] $($_.Exception.Message)"
    $script:failCount++
  }
}

Invoke-Step -Name "core-tests" -LogPath $coreLog -Action {
  Push-Location "core"
  try {
    cargo test -q
  } finally {
    Pop-Location
  }
}

Invoke-Step -Name "web-build" -LogPath $webLog -Action {
  Push-Location "web"
  try {
    npm run -s build
  } finally {
    Pop-Location
  }
}

Invoke-Step -Name "contract-checks" -LogPath $contractLog -Action {
  rg -n "enum AgentExecutionProfile|collision_policy|execution_options\(" "core/src/api_server.rs"
  rg -n "agentExecute\(planId, executionProfile\)|agentExecute\(planRes.plan_id, executionProfile\)|ExecutionProfile" "web/src/features/dashboard/Dashboard.tsx"
  rg -n "telegram_transport::send_message_chunked" "core/src/telegram.rs" "core/src/integrations/telegram.rs"
}

if ($env:STEER_RUN_PROFILE_MATRIX -ne "0") {
  Invoke-Step -Name "profile-matrix" -LogPath $profileMatrixLog -Action {
    $oldMinScore = $env:STEER_PROFILE_MATRIX_MIN_SCORE
    $oldSoftFail = $env:STEER_PROFILE_MATRIX_SOFT_FAIL
    $oldRequireBusiness = $env:STEER_PROFILE_MATRIX_REQUIRE_BUSINESS
    $oldProfiles = $env:STEER_PROFILE_MATRIX_PROFILES
    try {
      if ([string]::IsNullOrWhiteSpace($env:STEER_PROFILE_MATRIX_MIN_SCORE)) { $env:STEER_PROFILE_MATRIX_MIN_SCORE = "35" }
      if ([string]::IsNullOrWhiteSpace($env:STEER_PROFILE_MATRIX_SOFT_FAIL)) { $env:STEER_PROFILE_MATRIX_SOFT_FAIL = "0" }
      if ([string]::IsNullOrWhiteSpace($env:STEER_PROFILE_MATRIX_REQUIRE_BUSINESS)) { $env:STEER_PROFILE_MATRIX_REQUIRE_BUSINESS = "1" }
      if ([string]::IsNullOrWhiteSpace($env:STEER_PROFILE_MATRIX_PROFILES)) { $env:STEER_PROFILE_MATRIX_PROFILES = "fast" }
      & (Join-Path $PSScriptRoot "run_profile_matrix_regression.ps1")
      if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
        throw "run_profile_matrix_regression.ps1 exited with code $LASTEXITCODE"
      }
    } finally {
      $env:STEER_PROFILE_MATRIX_MIN_SCORE = $oldMinScore
      $env:STEER_PROFILE_MATRIX_SOFT_FAIL = $oldSoftFail
      $env:STEER_PROFILE_MATRIX_REQUIRE_BUSINESS = $oldRequireBusiness
      $env:STEER_PROFILE_MATRIX_PROFILES = $oldProfiles
    }
  }
}

if ($env:STEER_RUN_GUI_REGRESSION -eq "1") {
  $guiLog = Join-Path $outDir "gui_regression.log"
  Invoke-Step -Name "gui-regression-pack" -LogPath $guiLog -Action {
    & (Join-Path $PSScriptRoot "run_gui_regression_pack.ps1")
    if ($LASTEXITCODE -ne $null -and $LASTEXITCODE -ne 0) {
      throw "run_gui_regression_pack.ps1 exited with code $LASTEXITCODE"
    }
  }
}

$status = if ($failCount -eq 0) { "success" } else { "failed" }
@(
  "timestamp=$timestamp"
  "pass=$passCount"
  "fail=$failCount"
  "status=$status"
) | Set-Content -Path $summaryFile -Encoding UTF8

Write-Host ""
Write-Host "priority regression summary"
Write-Host " - pass: $passCount"
Write-Host " - fail: $failCount"
Write-Host " - out: $outDir"

if ($failCount -ne 0) {
  exit 1
}
exit 0
