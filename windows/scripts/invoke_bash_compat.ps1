param(
  [Parameter(Mandatory = $true)]
  [string]$ScriptPath,
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$ScriptArgs
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ScriptPath)) {
  throw "Target shell script not found: $ScriptPath"
}

$bashCandidates = @()
$bash = Get-Command bash -ErrorAction SilentlyContinue
if ($bash) {
  $bashCandidates += $bash.Source
}
$bashCandidates += @(
  "$env:ProgramFiles\Git\bin\bash.exe",
  "$env:ProgramFiles\Git\usr\bin\bash.exe",
  "$env:ProgramW6432\Git\bin\bash.exe"
)

$bashExe = $bashCandidates |
  Where-Object { $_ -and (Test-Path $_) } |
  Select-Object -First 1

if (-not $bashExe) {
  Write-Error "bash executable not found. Install Git for Windows or add bash to PATH."
  exit 127
}

& $bashExe $ScriptPath @ScriptArgs
exit $LASTEXITCODE
