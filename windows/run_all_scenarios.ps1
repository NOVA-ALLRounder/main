param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"

Write-Host "run_all_scenarios.ps1 is deprecated. Delegating to run_complex_scenarios.ps1"
$target = Join-Path $PSScriptRoot "run_complex_scenarios.ps1"
& $target @Args
exit $LASTEXITCODE
