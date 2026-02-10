param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$Recommendations = "collector\\Data-Collection-Projection\\logs\\workflow_recommendations.json",
  [string]$ConfigPath = "collector\\Data-Collection-Projection\\configs\\config.yaml",
  [string]$InputPath = "collector\\Data-Collection-Projection\\logs\\llm_input_hybrid.json",
  [string]$ProfilePath = "collector\\Data-Collection-Projection\\configs\\personalization_demo.json",
  [string]$ScoreConfig = "collector\\Data-Collection-Projection\\configs\\score_config.json",
  [int]$MinScore = 85,
  [int]$Candidates = 3,
  [string]$Id = "",
  [ValidateSet("approved", "rejected", "")]
  [string]$Status = "",
  [switch]$Apply,
  [switch]$NoActivate,
  [switch]$ListOnly
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$recPath = if (Test-Path $Recommendations) { $Recommendations } else { Join-Path $RepoPath $Recommendations }
if (-not (Test-Path $recPath)) {
  Write-Host "??recommendations not found: $recPath"
  exit 1
}

$recs = Get-Content $recPath -Raw | ConvertFrom-Json
if (-not $recs) {
  Write-Host "??recommendations empty"
  exit 1
}

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

if ($ListOnly) {
  exit 0
}

if (-not $Id) {
  $selection = Read-Host "Select 1-3 or id (Enter to cancel)"
  if (-not $selection) {
    Write-Host "??cancelled"
    exit 0
  }
  if ($selection -match '^\d+$') {
    $pick = [int]$selection - 1
    if ($pick -ge 0 -and $pick -lt $recs.Count) {
      $Id = $recs[$pick].id
    }
  } else {
    $Id = $selection.Trim()
  }
}

if (-not $Id) {
  Write-Host "??invalid selection"
  exit 1
}

if (-not $Status) {
  $action = Read-Host "Approve or Reject? (a/r)"
  if ($action -match '^(a|approve)$') {
    $Status = "approved"
  } elseif ($action -match '^(r|reject)$') {
    $Status = "rejected"
  }
}

if (-not $Status) {
  Write-Host "??invalid status"
  exit 1
}

$feedbackScript = Join-Path $RepoPath "collector\\Data-Collection-Projection\\scripts\\record_workflow_feedback.py"
if (-not (Test-Path $feedbackScript)) {
  Write-Host "??feedback script not found: $feedbackScript"
  exit 1
}

$conda = Get-Command conda -ErrorAction SilentlyContinue
if ($conda) {
  $condaExe = if ($conda.Path) { $conda.Path } else { "conda" }
}

$selected = $null
foreach ($rec in $recs) {
  if ($rec.id -eq $Id) { $selected = $rec; break }
}
$scoreArg = ""
$toolsArg = ""
if ($selected) {
  if ($selected.final_score) { $scoreArg = $selected.final_score } elseif ($selected.base_score) { $scoreArg = $selected.base_score }
  if ($selected.tools) { $toolsArg = ($selected.tools -join ",") }
}

if ($conda) {
  & $condaExe run -n DATA_C python $feedbackScript --id $Id --status $Status --source manual --score $scoreArg --tools $toolsArg --reasons "manual"
} else {
  python $feedbackScript --id $Id --status $Status --source manual --score $scoreArg --tools $toolsArg --reasons "manual"
}

Write-Host "??feedback_saved id=$Id status=$Status"

if ($Apply) {
  $dcpRoot = Join-Path $RepoPath "collector\\Data-Collection-Projection"
  $logDir = Join-Path $dcpRoot "logs"
  $workflowFinal = Join-Path $logDir "n8n_workflow.json"
  $genScript = Join-Path $dcpRoot "scripts\\generate_workflow_recommendations.py"
  if (-not (Test-Path $genScript)) {
    Write-Host "??generator not found: $genScript"
    exit 1
  }
  Write-Host "??Regenerating recommendations..."
  if ($conda) {
    & $condaExe run -n DATA_C python $genScript `
      --config $ConfigPath --input $InputPath --output-dir $logDir `
      --profile $ProfilePath --score-config $ScoreConfig --min-score $MinScore --candidates $Candidates `
      --save-best $workflowFinal --include-template
  } else {
    python $genScript --config $ConfigPath --input $InputPath --output-dir $logDir `
      --profile $ProfilePath --score-config $ScoreConfig --min-score $MinScore --candidates $Candidates `
      --save-best $workflowFinal --include-template
  }

  $bootstrap = Join-Path $RepoPath "scripts\\n8n_bootstrap.ps1"
  if (Test-Path $bootstrap) {
    Write-Host "??Importing updated workflow into n8n..."
    if ($NoActivate) {
      & $bootstrap -NoActivate
    } else {
      & $bootstrap
    }
  }
}
