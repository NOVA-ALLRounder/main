param(
  [Parameter(Position = 0, Mandatory = $true)][string]$Message,
  [Parameter(Position = 1)][string]$ImagePath = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Set-Location $PSScriptRoot

function Get-EnvValue {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$Default
  )
  $value = [Environment]::GetEnvironmentVariable($Name)
  if ([string]::IsNullOrWhiteSpace($value)) {
    return $Default
  }
  return $value
}

function Get-EnvInt {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][int]$Default
  )
  $raw = Get-EnvValue -Name $Name -Default "$Default"
  $parsed = 0
  if ([int]::TryParse($raw, [ref]$parsed)) {
    return $parsed
  }
  return $Default
}

function Is-Truthy {
  param([string]$Value)
  if ([string]::IsNullOrWhiteSpace($Value)) {
    return $false
  }
  return ($Value.Trim().ToLowerInvariant() -in @("1", "true", "yes", "on"))
}

function Csv-Contains {
  param(
    [Parameter(Mandatory = $true)][string]$Value,
    [Parameter(Mandatory = $true)][string]$Csv
  )
  if ([string]::IsNullOrWhiteSpace($Csv)) {
    return $false
  }
  $normalized = $Value.Trim().ToLowerInvariant()
  foreach ($token in ($Csv -split ",")) {
    if ($token.Trim().ToLowerInvariant() -eq $normalized) {
      return $true
    }
  }
  return $false
}

function Should-SendTelegram {
  param(
    [Parameter(Mandatory = $true)][string]$ChatId,
    [Parameter(Mandatory = $true)][string]$Text
  )
  $policy = Get-EnvValue -Name "NOTIFY_POLICY" -Default "allow"
  if ($policy.Trim().ToLowerInvariant() -eq "deny") {
    Write-Host "Telegram suppressed by NOTIFY_POLICY=deny"
    return $false
  }

  if (Csv-Contains -Value "telegram" -Csv (Get-EnvValue -Name "NOTIFY_DENY_CHANNELS" -Default "")) {
    Write-Host "Telegram suppressed by NOTIFY_DENY_CHANNELS"
    return $false
  }

  $allowChannels = Get-EnvValue -Name "NOTIFY_ALLOW_CHANNELS" -Default ""
  if (-not [string]::IsNullOrWhiteSpace($allowChannels) -and -not (Csv-Contains -Value "telegram" -Csv $allowChannels)) {
    Write-Host "Telegram suppressed by NOTIFY_ALLOW_CHANNELS"
    return $false
  }

  $denyTargets = Get-EnvValue -Name "NOTIFY_DENY_TARGET_IDS" -Default (Get-EnvValue -Name "NOTIFY_DENY_CHAT_IDS" -Default "")
  if (Csv-Contains -Value $ChatId -Csv $denyTargets) {
    Write-Host "Telegram suppressed by NOTIFY_DENY_TARGET_IDS"
    return $false
  }

  $allowTargets = Get-EnvValue -Name "NOTIFY_ALLOW_TARGET_IDS" -Default (Get-EnvValue -Name "NOTIFY_ALLOW_CHAT_IDS" -Default "")
  if (-not [string]::IsNullOrWhiteSpace($allowTargets) -and -not (Csv-Contains -Value $ChatId -Csv $allowTargets)) {
    Write-Host "Telegram suppressed by NOTIFY_ALLOW_TARGET_IDS"
    return $false
  }

  return $true
}

function Split-MessageChunks {
  param(
    [Parameter(Mandatory = $true)][string]$Text,
    [Parameter(Mandatory = $true)][int]$MaxLen
  )
  $chunks = New-Object System.Collections.Generic.List[string]
  $remaining = $Text

  while ($remaining.Length -gt $MaxLen) {
    $cut = $remaining.LastIndexOf("`n", $MaxLen)
    if ($cut -lt [Math]::Floor($MaxLen * 0.6)) {
      $cut = $MaxLen
    }
    $chunk = $remaining.Substring(0, $cut).TrimEnd()
    if (-not [string]::IsNullOrWhiteSpace($chunk)) {
      [void]$chunks.Add($chunk)
    }
    $remaining = $remaining.Substring($cut).TrimStart("`r", "`n")
  }

  if (-not [string]::IsNullOrWhiteSpace($remaining)) {
    [void]$chunks.Add($remaining)
  }
  return @($chunks)
}

function Send-TelegramTextOnce {
  param(
    [Parameter(Mandatory = $true)][string]$BotToken,
    [Parameter(Mandatory = $true)][string]$ChatId,
    [Parameter(Mandatory = $true)][string]$Text,
    [Parameter(Mandatory = $true)][int]$TimeoutSec
  )
  $uri = "https://api.telegram.org/bot$BotToken/sendMessage"
  $payload = @{
    chat_id = $ChatId
    text = $Text
    disable_web_page_preview = "true"
  }
  $resp = Invoke-RestMethod -Method Post -Uri $uri -Body $payload -TimeoutSec $TimeoutSec
  $ok = $resp.PSObject.Properties["ok"]
  return ($null -ne $ok -and [bool]$ok.Value)
}

function Send-TelegramText {
  param(
    [Parameter(Mandatory = $true)][string]$BotToken,
    [Parameter(Mandatory = $true)][string]$ChatId,
    [Parameter(Mandatory = $true)][string]$Text,
    [Parameter(Mandatory = $true)][int]$RetryCount,
    [Parameter(Mandatory = $true)][int]$RetryDelaySec,
    [Parameter(Mandatory = $true)][int]$TimeoutSec,
    [Parameter(Mandatory = $true)][int]$MaxLen
  )

  $chunks = Split-MessageChunks -Text $Text -MaxLen $MaxLen
  if ($chunks.Count -eq 0) {
    $chunks = @("")
  }

  foreach ($chunk in $chunks) {
    $sent = $false
    for ($attempt = 1; $attempt -le $RetryCount; $attempt++) {
      try {
        if (Send-TelegramTextOnce -BotToken $BotToken -ChatId $ChatId -Text $chunk -TimeoutSec $TimeoutSec) {
          $sent = $true
          break
        }
      } catch {
        # retry
      }
      if ($attempt -lt $RetryCount) {
        $sleepSec = [Math]::Min(30, [int]($RetryDelaySec * [Math]::Pow(2, $attempt - 1)))
        Start-Sleep -Seconds $sleepSec
      }
    }
    if (-not $sent) {
      return $false
    }
  }
  return $true
}

function Send-TelegramPhoto {
  param(
    [Parameter(Mandatory = $true)][string]$BotToken,
    [Parameter(Mandatory = $true)][string]$ChatId,
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Caption,
    [Parameter(Mandatory = $true)][int]$RetryCount,
    [Parameter(Mandatory = $true)][int]$RetryDelaySec
  )

  if (-not (Test-Path $Path)) {
    Write-Host "Image file not found: $Path"
    return $false
  }

  $url = "https://api.telegram.org/bot$BotToken/sendPhoto"
  $finalCaption = $Caption
  if ($finalCaption.Length -gt 900) {
    $finalCaption = $finalCaption.Substring(0, 900) + "..."
  }

  for ($attempt = 1; $attempt -le $RetryCount; $attempt++) {
    $respText = & curl.exe -sS -X POST $url -F "chat_id=$ChatId" -F "photo=@$Path" -F "caption=$finalCaption"
    if ($LASTEXITCODE -eq 0) {
      try {
        $resp = $respText | ConvertFrom-Json
        if ($resp.ok -eq $true) {
          return $true
        }
      } catch {
        # retry
      }
    }

    if ($attempt -lt $RetryCount) {
      $sleepSec = [Math]::Min(30, [int]($RetryDelaySec * [Math]::Pow(2, $attempt - 1)))
      Start-Sleep -Seconds $sleepSec
    }
  }

  return $false
}

$botToken = Get-EnvValue -Name "TELEGRAM_BOT_TOKEN" -Default ""
$chatId = Get-EnvValue -Name "TELEGRAM_CHAT_ID" -Default ""
$retryCount = [Math]::Max(1, (Get-EnvInt -Name "TELEGRAM_RETRY_COUNT" -Default 3))
$retryDelaySec = [Math]::Max(1, (Get-EnvInt -Name "TELEGRAM_RETRY_DELAY_SEC" -Default 1))
$timeoutSec = [Math]::Max(5, (Get-EnvInt -Name "TELEGRAM_MAX_TIME" -Default 20))
$maxTextLen = [Math]::Max(500, (Get-EnvInt -Name "TELEGRAM_MAX_TEXT_LEN" -Default 3800))
$requireSend = Is-Truthy (Get-EnvValue -Name "TELEGRAM_REQUIRE_SEND" -Default "0")

if ([string]::IsNullOrWhiteSpace($botToken) -or [string]::IsNullOrWhiteSpace($chatId)) {
  Write-Host "TELEGRAM_BOT_TOKEN and TELEGRAM_CHAT_ID must be set"
  exit 1
}

if (-not (Should-SendTelegram -ChatId $chatId -Text $Message)) {
  if ($requireSend) {
    Write-Host "Telegram blocked by policy while TELEGRAM_REQUIRE_SEND=1"
    exit 1
  }
  exit 0
}

$skipRewrite = Is-Truthy (Get-EnvValue -Name "TELEGRAM_SKIP_REWRITE" -Default "0")
if (-not $skipRewrite) {
  $binaryCandidates = @(
    (Join-Path $PSScriptRoot "core\\target\\debug\\local_os_agent.exe"),
    (Join-Path $PSScriptRoot "core\\target\\debug\\local_os_agent")
  )
  foreach ($candidate in $binaryCandidates) {
    if (Test-Path $candidate) {
      try {
        $rewritten = (& $candidate rewrite $Message 2>$null)
        if (-not [string]::IsNullOrWhiteSpace($rewritten)) {
          $Message = $rewritten
        }
      } catch {
        # keep original message
      }
      break
    }
  }
}

$dumpPath = Get-EnvValue -Name "TELEGRAM_DUMP_FINAL_PATH" -Default ""
if (-not [string]::IsNullOrWhiteSpace($dumpPath)) {
  $dumpDir = Split-Path -Parent $dumpPath
  if (-not [string]::IsNullOrWhiteSpace($dumpDir)) {
    New-Item -ItemType Directory -Force -Path $dumpDir | Out-Null
  }
  $Message | Set-Content -Path $dumpPath -Encoding UTF8
}

$sent = $false
if (-not [string]::IsNullOrWhiteSpace($ImagePath)) {
  $sent = Send-TelegramPhoto -BotToken $botToken -ChatId $chatId -Path $ImagePath -Caption $Message -RetryCount $retryCount -RetryDelaySec $retryDelaySec
  if ($sent -and $Message.Length -gt 900) {
    $sent = Send-TelegramText -BotToken $botToken -ChatId $chatId -Text $Message -RetryCount $retryCount -RetryDelaySec $retryDelaySec -TimeoutSec $timeoutSec -MaxLen $maxTextLen
  }
} else {
  $sent = Send-TelegramText -BotToken $botToken -ChatId $chatId -Text $Message -RetryCount $retryCount -RetryDelaySec $retryDelaySec -TimeoutSec $timeoutSec -MaxLen $maxTextLen
}

if (-not $sent) {
  Write-Host "Telegram notification failed"
  exit 1
}

$extraListFile = Get-EnvValue -Name "TELEGRAM_EXTRA_IMAGE_LIST_FILE" -Default ""
$extraImageMax = [Math]::Max(0, (Get-EnvInt -Name "TELEGRAM_EXTRA_IMAGE_MAX" -Default 1))
if ($extraImageMax -gt 0 -and -not [string]::IsNullOrWhiteSpace($extraListFile) -and (Test-Path $extraListFile)) {
  $entries = Get-Content -Path $extraListFile | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  if ($entries.Count -gt $extraImageMax) {
    $entries = $entries[($entries.Count - $extraImageMax)..($entries.Count - 1)]
  }
  foreach ($entry in $entries) {
    $parts = $entry.Split("|", 2)
    $extraPath = $parts[0].Trim()
    $extraCaption = if ($parts.Count -gt 1) { $parts[1] } else { "Node evidence" }
    if (-not (Send-TelegramPhoto -BotToken $botToken -ChatId $chatId -Path $extraPath -Caption $extraCaption -RetryCount $retryCount -RetryDelaySec $retryDelaySec)) {
      Write-Host "Telegram extra image failed: $extraPath"
      exit 1
    }
  }
}

Write-Host "Telegram notification sent"
exit 0
