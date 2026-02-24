param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [ValidateSet("dev","build","preview")] [string]$Mode = "dev",
  [switch]$Install
)

$ErrorActionPreference = "Stop"

$webDir = Join-Path $RepoPath "web"
if (-not (Test-Path $webDir)) {
  throw "web directory not found: $webDir"
}

Set-Location $webDir

if ($Install -or -not (Test-Path (Join-Path $webDir "node_modules"))) {
  & npm install
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

switch ($Mode) {
  "dev" {
    & npm run dev
  }
  "build" {
    & npm run build
  }
  "preview" {
    & npm run preview
  }
}

exit $LASTEXITCODE
