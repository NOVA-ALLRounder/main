param(
  [string]$ApiBase = "http://127.0.0.1:5680"
)

$ErrorActionPreference = "Stop"

Write-Host "[smoke] checking $ApiBase/api/health"
$health = Invoke-RestMethod -Uri "$ApiBase/api/health" -Method Get
if ($health -ne "ok") {
  throw "health check failed: $health"
}
Write-Host "[ok] health"

Write-Host "[smoke] checking $ApiBase/api/context/selection"
$selection = Invoke-RestMethod -Uri "$ApiBase/api/context/selection" -Method Get
if ($null -eq $selection.found) {
  throw "selection payload missing 'found'"
}
Write-Host "[ok] selection endpoint"

Write-Host "[smoke] checking $ApiBase/api/system/health"
$systemHealth = Invoke-RestMethod -Uri "$ApiBase/api/system/health" -Method Get
if ($null -eq $systemHealth) {
  throw "system health payload is empty"
}
Write-Host "[ok] system health endpoint"

Write-Host "[done] core api smoke passed"
