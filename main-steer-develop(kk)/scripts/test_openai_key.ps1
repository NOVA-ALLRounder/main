param(
    [string]$EnvPath = "core/.env",
    [int]$TimeoutSec = 15
)

$ErrorActionPreference = "Stop"

function Get-EnvValue {
    param(
        [string]$Path,
        [string]$Name
    )

    if (-not (Test-Path $Path)) {
        return $null
    }

    $line = Get-Content $Path | Where-Object { $_ -match "^\s*$Name\s*=" } | Select-Object -First 1
    if (-not $line) {
        return $null
    }

    return ($line -replace "^\s*$Name\s*=\s*", "").Trim()
}

$resolvedEnvPath = if ([System.IO.Path]::IsPathRooted($EnvPath)) { $EnvPath } else { Join-Path (Get-Location) $EnvPath }
$apiKey = Get-EnvValue -Path $resolvedEnvPath -Name "OPENAI_API_KEY"

if (-not $apiKey) {
    Write-Output "RESULT=FAIL REASON=KEY_MISSING FILE=$resolvedEnvPath"
    exit 1
}

$prefix = if ($apiKey.Length -ge 7) { $apiKey.Substring(0, 7) } else { $apiKey }
Write-Output "KEY=FOUND PREFIX=$prefix LEN=$($apiKey.Length)"

$uri = "https://api.openai.com/v1/models"

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $resp = Invoke-WebRequest -UseBasicParsing -Method Get -Uri $uri -Headers @{ Authorization = "Bearer $apiKey" } -TimeoutSec $TimeoutSec
    if ($resp.StatusCode -ge 200 -and $resp.StatusCode -lt 300) {
        Write-Output "RESULT=OK METHOD=Invoke-WebRequest STATUS=$($resp.StatusCode)"
        exit 0
    }
    Write-Output "RESULT=FAIL METHOD=Invoke-WebRequest STATUS=$($resp.StatusCode)"
    exit 2
} catch {
    if ($_.Exception.Response) {
        $status = [int]$_.Exception.Response.StatusCode
        if ($status -eq 401) {
            Write-Output "RESULT=INVALID_KEY METHOD=Invoke-WebRequest STATUS=401"
            exit 3
        }
        Write-Output "RESULT=HTTP_ERROR METHOD=Invoke-WebRequest STATUS=$status"
        exit 4
    }
    Write-Output "RESULT=NETWORK_ERROR METHOD=Invoke-WebRequest MESSAGE=$($_.Exception.Message)"
}

# Fallback: curl.exe
try {
    $headers = @("Authorization: Bearer $apiKey")
    $args = @("-sS", "-o", "NUL", "-w", "%{http_code}", "--connect-timeout", "$TimeoutSec", "--max-time", "$TimeoutSec", "-H", $headers[0], $uri)
    $code = & curl.exe @args
    if ($LASTEXITCODE -ne 0) {
        Write-Output "RESULT=NETWORK_ERROR METHOD=curl EXIT=$LASTEXITCODE"
        exit 5
    }
    if ($code -eq "200") {
        Write-Output "RESULT=OK METHOD=curl STATUS=200"
        exit 0
    }
    if ($code -eq "401") {
        Write-Output "RESULT=INVALID_KEY METHOD=curl STATUS=401"
        exit 3
    }
    Write-Output "RESULT=HTTP_ERROR METHOD=curl STATUS=$code"
    exit 4
} catch {
    Write-Output "RESULT=NETWORK_ERROR METHOD=curl MESSAGE=$($_.Exception.Message)"
    exit 5
}
