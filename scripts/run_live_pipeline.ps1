param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [int]$EverySeconds = 300,
  [int]$WindowMinutes = 10,
  [int]$MinScore = 85,
  [int]$MaxAttempts = 3,
  [int]$MaxBytes = 8000,
  [int]$MaxTopApps = 5,
  [int]$MaxTitles = 5,
  [int]$MaxEvents = 8,
  [int]$MaxSamples = 12,
  [int]$MaxSequence = 12,
  [int]$MaxTransitions = 8,
  [int]$PatternEveryMinutes = 60,
  [int]$PatternWindowDays = 7,
  [Alias("AutoApproveScore")]
  [int]$AutoApproveMinScore = 110,
  [string]$AutoApproveRequireSource = "llm",
  [int]$AutoApproveMinToolMatches = 1,
  [switch]$Aggressive,
  [switch]$AutoApprove,
  [switch]$AutoApproveRequireRationale,
  [switch]$AutoApproveRequireSequence,
  [ValidateSet("strict", "balanced", "permissive")]
  [string]$CompressionLevel = "balanced",
  [string]$PreserveFields = "",
  [switch]$NoQualityDashboard,
  [switch]$NoDataLevels,
  [switch]$NoActivate,
  [switch]$SkipCollector
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$dcpRoot = Join-Path $RepoPath "collector\\Data-Collection-Projection"
$configPath = Join-Path $dcpRoot "configs\\config.yaml"
$profilePath = Join-Path $dcpRoot "configs\\personalization_demo.json"
$scorePath = Join-Path $dcpRoot "configs\\score_config.json"
$feedbackPath = Join-Path $dcpRoot "configs\\workflow_feedback.json"
$logDir = Join-Path $dcpRoot "logs"
$workflowHybrid = Join-Path $logDir "n8n_workflow_hybrid.json"
$workflowFinal = Join-Path $logDir "n8n_workflow.json"
$hashFile = Join-Path $logDir "last_workflow.sha256"
$patternStamp = Join-Path $logDir "last_pattern_update.txt"
$qualityScript = Join-Path $dcpRoot "scripts\\quality_dashboard.py"
$partitionScript = Join-Path $dcpRoot "scripts\\partition_data_levels.py"

function Normalize-Tool([string]$Tool) {
  if (-not $Tool) { return "" }
  $t = $Tool.ToLower().Trim()
  if ($t -match "kakao") { return "kakao" }
  if ($t -match "google" -and $t -match "calendar") { return "googlecalendar" }
  if ($t -match "calendar") { return "googlecalendar" }
  if ($t -match "gmail" -or $t -eq "email" -or $t -eq "mail") { return "gmail" }
  if ($t -match "notion") { return "notion" }
  if ($t -match "slack") { return "slack" }
  return $t
}

function Get-PreferredTools([string]$ProfilePath) {
  if (-not (Test-Path $ProfilePath)) { return @() }
  try {
    $profile = Get-Content $ProfilePath -Raw | ConvertFrom-Json
  } catch {
    return @()
  }
  $tools = @()
  if ($profile.preferred_actions) {
    foreach ($item in $profile.preferred_actions) {
      if ($item.tool) {
        $tools += (Normalize-Tool $item.tool)
      } elseif ($item -is [string]) {
        $tools += (Normalize-Tool $item)
      }
    }
  }
  if (-not $tools -and $profile.safety -and $profile.safety.allow_actions) {
    foreach ($item in $profile.safety.allow_actions) {
      $tools += (Normalize-Tool $item)
    }
  }
  $tools = $tools | Where-Object { $_ } | Select-Object -Unique
  return ,$tools
}

function Evaluate-AutoApprove($Rec, $ProfilePath) {
  $result = @{ should = $true; reasons = @(); failed = @(); score = 0 }
  if (-not $Rec) { $result.should = $false; $result.failed += "no_rec"; return $result }
  $score = if ($Rec.final_score) { [int]$Rec.final_score } else { [int]$Rec.base_score }
  $result.score = $score
  $result.reasons += ("score>=" + $AutoApproveMinScore)
  if ($AutoApproveRequireSource) { $result.reasons += ("source=" + $AutoApproveRequireSource) }
  if ($AutoApproveMinToolMatches -gt 0) { $result.reasons += ("tool_matches>=" + $AutoApproveMinToolMatches) }
  if ($AutoApproveRequireRationale) { $result.reasons += "rationale_present" }
  if ($AutoApproveRequireSequence) { $result.reasons += "sequence_present" }

  if ($score -lt $AutoApproveMinScore) { $result.should = $false; $result.failed += "score" }
  if ($AutoApproveRequireSource -and $Rec.source -ne $AutoApproveRequireSource) { $result.should = $false; $result.failed += "source" }
  if ($AutoApproveRequireRationale -and -not $Rec.rationale) { $result.should = $false; $result.failed += "rationale" }

  if ($AutoApproveRequireSequence -and $Rec.rationale) {
    $r = ($Rec.rationale -join "; ")
    if ($r -notmatch "sequence" -and $r -notmatch "pattern_sequence") { $result.should = $false; $result.failed += "sequence" }
  }

  $preferredTools = Get-PreferredTools $ProfilePath
  if ($AutoApproveMinToolMatches -gt 0 -and $Rec.tools) {
    $tools = @($Rec.tools | ForEach-Object { Normalize-Tool $_ })
    $matches = @($tools | Where-Object { $preferredTools -contains $_ }).Count
    if ($matches -lt $AutoApproveMinToolMatches) { $result.should = $false; $result.failed += "tools" }
  }
  return $result
}

function Write-QualityEvent([string]$Type, [hashtable]$Data) {
  try {
    $event = @{ ts = (Get-Date).ToUniversalTime().ToString("o"); type = $Type }
    if ($Data) {
      foreach ($key in $Data.Keys) {
        $event[$key] = $Data[$key]
      }
    }
    $json = $event | ConvertTo-Json -Compress -Depth 6
    $path = Join-Path $logDir "quality_events.jsonl"
    Add-Content -Path $path -Value $json
  } catch {
    Write-Host "??Failed to write quality event"
  }
}

if (-not (Test-Path $configPath)) {
  Write-Host "❌ Config not found: $configPath"
  exit 1
}

if ($Aggressive) {
  $MaxBytes = 4000
  $MaxTopApps = 4
  $MaxTitles = 4
  $MaxEvents = 6
  $MaxSamples = 6
  $MaxSequence = 8
  $MaxTransitions = 5
  if ($CompressionLevel -eq "balanced") {
    $CompressionLevel = "strict"
  }
}

if (-not $SkipCollector) {
  Write-Host "▶ Starting DCP + sensors..."
  & (Join-Path $RepoPath "scripts\\run_all.ps1") | Out-Null
}

Write-Host "▶ Ensuring n8n is running..."
docker compose -f (Join-Path $RepoPath "docker-compose.yml") up -d n8n | Out-Null

while ($true) {
  $now = Get-Date
  $lastPattern = if (Test-Path $patternStamp) { Get-Content $patternStamp -Raw } else { "" }
  $shouldPattern = $true
  if ($lastPattern) {
    try {
      $lastDt = [datetime]::Parse($lastPattern)
      $elapsed = ($now - $lastDt).TotalMinutes
      if ($elapsed -lt $PatternEveryMinutes) { $shouldPattern = $false }
    } catch {
      $shouldPattern = $true
    }
  }

  if ($shouldPattern) {
    Write-Host "▶ Updating pattern summary ($PatternWindowDays days)..."
    conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\build_pattern_summary.py") `
      --summaries-dir $logDir --since-days $PatternWindowDays --config $configPath --store-db
    $now.ToString("o") | Set-Content $patternStamp
  }

  Write-Host "▶ Building realtime + hybrid input..."
  conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\build_realtime_llm_input.py") `
    --config $configPath --since-minutes $WindowMinutes --output (Join-Path $logDir "llm_input_realtime.json") `
    --max-bytes $MaxBytes --max-top-apps $MaxTopApps --max-titles $MaxTitles --max-events $MaxEvents `
    --max-samples $MaxSamples --max-sequence $MaxSequence --max-transitions $MaxTransitions `
    --min-duration-sec 5 --drop-idle --summary-only `
    --compression-level $CompressionLevel --preserve-fields $PreserveFields

  conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\build_hybrid_llm_input.py") `
    --output (Join-Path $logDir "llm_input_hybrid.json") --realtime (Join-Path $logDir "llm_input_realtime.json") `
    --max-bytes $MaxBytes `
    --compression-level $CompressionLevel --preserve-fields $PreserveFields

  Write-Host "▶ Generating workflow recommendations..."
  conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\generate_workflow_recommendations.py") `
    --config $configPath --input (Join-Path $logDir "llm_input_hybrid.json") --output-dir $logDir `
    --profile $profilePath --score-config $scorePath --min-score $MinScore --candidates $MaxAttempts `
    --save-best $workflowFinal --include-template
  $recExit = $LASTEXITCODE
  if ($recExit -ne 0) {
    Write-QualityEvent -Type "recommendations_failed" -Data @{ code = $recExit }
  }



  $recPath = Join-Path $logDir "workflow_recommendations.json"
  $topRec = $null
  $rerunRecommendations = $false
  if (Test-Path $recPath) {
    try {
      $recs = Get-Content $recPath -Raw | ConvertFrom-Json
      if ($recs) {
        $recs = @($recs)
        $hadRecs = $false
        Write-Host "??Top Recommendations:"
        $idx = 1
        foreach ($rec in $recs) {
          $tools = if ($rec.tools) { ($rec.tools -join ",") } else { "none" }
          $score = if ($rec.final_score) { $rec.final_score } else { $rec.base_score }
          Write-Host ("  {0}. {1} score={2} tools={3}" -f $idx, $rec.id, $score, $tools)
          if ($rec.rationale) {
            Write-Host ("     rationale: {0}" -f ($rec.rationale -join "; "))
          }
          $idx++
        }
        if ($recs.Count -gt 0) {
          $topRec = $recs[0]
          $hadRecs = $true
        }
        if ($AutoApprove -and $topRec) {
          $check = Evaluate-AutoApprove $topRec $profilePath
          $status = if ($check.should) { 'PASS' } else { 'FAIL' }
          $fail = if ($check.failed.Count -gt 0) { ($check.failed -join ',') } else { 'none' }
          Write-Host ("     auto_approve: {0} reasons={1} failed={2}" -f $status, ($check.reasons -join '; '), $fail)
        }
        if (-not $hadRecs) {
          Write-QualityEvent -Type "no_candidates" -Data @{ reason = "empty_recommendations" }
        }
      } else {
        Write-QualityEvent -Type "no_candidates" -Data @{ reason = "recommendations_null" }
      }
    } catch {
      Write-Host "??Failed to read workflow_recommendations.json"
      Write-QualityEvent -Type "recommendations_parse_failed" -Data @{ path = $recPath }
    }
  } else {
    Write-QualityEvent -Type "no_candidates" -Data @{ reason = "recommendations_missing" }
  }

  if ($AutoApprove -and $topRec) {
    $check = Evaluate-AutoApprove $topRec $profilePath
    if ($check.should) {
      $approved = @()
      $rejected = @()
      if (Test-Path $feedbackPath) {
        try {
          $fb = Get-Content $feedbackPath -Raw | ConvertFrom-Json
          if ($fb.approved) { $approved = @($fb.approved) }
          if ($fb.rejected) { $rejected = @($fb.rejected) }
        } catch {}
      }
      if (($approved -notcontains $topRec.id) -and ($rejected -notcontains $topRec.id)) {
        Write-Host ("??Auto-approving: {0} score={1}" -f $topRec.id, $check.score)
        conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\record_workflow_feedback.py") `
          --id $topRec.id --status approved --source auto --score $check.score --tools ($topRec.tools -join ",") `
          --reasons ($check.reasons -join ";")
        Write-QualityEvent -Type "auto_approved" -Data @{ id = $topRec.id; score = $check.score; tools = $topRec.tools; reasons = $check.reasons }
        $rerunRecommendations = $true
      }
    }
  }

  if ($rerunRecommendations) {
    Write-Host "??Rebuilding recommendations after auto-approval..."
    conda run -n DATA_C python (Join-Path $dcpRoot "scripts\\generate_workflow_recommendations.py") `
      --config $configPath --input (Join-Path $logDir "llm_input_hybrid.json") --output-dir $logDir `
      --profile $profilePath --score-config $scorePath --min-score $MinScore --candidates $MaxAttempts `
      --save-best $workflowFinal --include-template
  }

  if (-not $NoQualityDashboard -and (Test-Path $qualityScript)) {
    conda run -n DATA_C python $qualityScript --log-dir $logDir
  }

  if (-not $NoDataLevels -and (Test-Path $partitionScript)) {
    conda run -n DATA_C python $partitionScript --log-dir $logDir
  }

  if (Test-Path $workflowFinal) {
    $hash = (Get-FileHash -Path $workflowFinal -Algorithm SHA256).Hash
    $prev = if (Test-Path $hashFile) { Get-Content $hashFile -Raw } else { "" }
    if ($hash -ne $prev) {
      if (-not $NoActivate) {
        $payload = Get-Content $workflowFinal -Raw | ConvertFrom-Json
        $payload.active = $true
        $payload | ConvertTo-Json -Depth 40 | Set-Content -Path $workflowFinal
      }
      Write-Host "▶ Importing workflow into n8n (CLI)..."
      $container = "main-n8n-1"
      docker cp $workflowFinal "${container}:/tmp/n8n_workflow.json" | Out-Null
      docker exec $container n8n import:workflow --input "/tmp/n8n_workflow.json" | Out-Null
      $importExit = $LASTEXITCODE
      $hash | Set-Content $hashFile
      Write-Host "✅ Workflow imported."
      if ($importExit -ne 0) {
        Write-QualityEvent -Type "import_failed" -Data @{ method = "cli"; code = $importExit }
      } else {
        Write-QualityEvent -Type "workflow_imported" -Data @{ method = "cli"; hash = $hash }
      }
    } else {
      Write-Host "⏭️  Workflow unchanged. Skipping import."
      Write-QualityEvent -Type "workflow_unchanged" -Data @{ hash = $hash }
    }
  }

  Start-Sleep -Seconds $EverySeconds
}
