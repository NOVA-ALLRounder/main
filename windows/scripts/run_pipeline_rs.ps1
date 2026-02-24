param(
  [string]$RepoPath = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$ConfigPath = "configs\\config.yaml",
  [int]$SinceHours = 6,
  [int]$Days = 3,
  [int]$MinSupport = 2
)

$ErrorActionPreference = "Stop"
Set-Location $RepoPath

$resolvedConfig = $ConfigPath
if (-not (Test-Path $resolvedConfig)) {
  $resolvedConfig = Join-Path $RepoPath $ConfigPath
}
if (-not (Test-Path $resolvedConfig)) {
  throw "Config not found: $resolvedConfig"
}

$manifestPath = Join-Path $RepoPath "core\\Cargo.toml"
$binDir = Join-Path $RepoPath "core\\target\\debug"
$sessionsBin = Join-Path $binDir "build_sessions_rs.exe"
$routinesBin = Join-Path $binDir "build_routines_rs.exe"
$handoffBin = Join-Path $binDir "build_handoff_rs.exe"

if (-not (Test-Path $sessionsBin) -or -not (Test-Path $routinesBin) -or -not (Test-Path $handoffBin)) {
  & cargo build --manifest-path $manifestPath --bin build_sessions_rs --bin build_routines_rs --bin build_handoff_rs
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

& $sessionsBin --config $resolvedConfig --since-hours $SinceHours --use-state
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $routinesBin --config $resolvedConfig --days $Days --min-support $MinSupport --use-state
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

& $handoffBin --config $resolvedConfig --skip-unchanged --keep-latest-pending
exit $LASTEXITCODE
