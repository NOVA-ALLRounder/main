param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..\").Path,
  [string]$ComposeFile = "docker-compose.yml",
  [string]$ApiUrl = "http://localhost:5678",
  [string]$WorkflowPath = "collector\\Data-Collection-Projection\\logs\\n8n_workflow.json",
  [switch]$Activate,
  [switch]$NoActivate
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

if ($Activate) { $NoActivate = $false }

$env:N8N_API_URL = $ApiUrl
if (-not $env:N8N_BASIC_AUTH_USER) { $env:N8N_BASIC_AUTH_USER = "admin" }
if (-not $env:N8N_BASIC_AUTH_PASSWORD) { $env:N8N_BASIC_AUTH_PASSWORD = "admin123" }

Write-Host "Starting n8n via Docker..."
docker compose -f $ComposeFile up -d n8n | Out-Null

Write-Host "Waiting for n8n on $ApiUrl ..."
$ready = $false
for ($i = 0; $i -lt 60; $i++) {
  try {
    $ready = Test-NetConnection -ComputerName "localhost" -Port 5678 -InformationLevel Quiet
  } catch {
    $ready = $false
  }
  if ($ready) { break }
  Start-Sleep -Seconds 2
}

if (-not $ready) {
  Write-Host "n8n not reachable on port 5678"
  exit 1
}

$importFile = $WorkflowPath
if (-not $NoActivate) {
  $activePath = Join-Path (Split-Path $WorkflowPath) "n8n_workflow_active.json"
  $payload = Get-Content $WorkflowPath -Raw | ConvertFrom-Json
  $payload.active = $true
  $payload | ConvertTo-Json -Depth 40 | Set-Content -Path $activePath
  $importFile = $activePath
}

Write-Host "Importing workflow via n8n CLI (container)..."
$container = "main-n8n-1"
$tempPath = "/tmp/n8n_workflow.json"
docker cp $importFile "${container}:$tempPath" | Out-Null
docker exec $container n8n import:workflow --input "$tempPath" | Out-Null
Write-Host "CLI import completed."
