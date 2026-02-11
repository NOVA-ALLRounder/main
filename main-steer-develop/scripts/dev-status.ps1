param(
  [int]$CorePort = 5680,
  [int]$WebPort = 5173
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Continue"

function Get-StatusCode {
  param([string]$Url)
  try {
    $resp = Invoke-WebRequest -Uri $Url -Method Get -UseBasicParsing -TimeoutSec 3
    return [int]$resp.StatusCode
  } catch {
    return -1
  }
}

$coreHealthUrl = "http://127.0.0.1:$CorePort/api/health"
$webUrl = "http://127.0.0.1:$WebPort"

$coreCode = Get-StatusCode -Url $coreHealthUrl
$webCode = Get-StatusCode -Url $webUrl

if ($coreCode -gt 0) {
  Write-Host "[OK] Core up ($coreCode): $coreHealthUrl"
} else {
  Write-Host "[X] Core down: $coreHealthUrl"
}

if ($webCode -gt 0) {
  Write-Host "[OK] Web up ($webCode): $webUrl"
} else {
  Write-Host "[X] Web down: $webUrl"
}

if (($coreCode -gt 0) -and ($webCode -gt 0)) {
  exit 0
}
exit 1
