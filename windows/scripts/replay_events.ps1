param(
  [string]$ApiUrl = $(if ($env:API_URL) { $env:API_URL } else { "http://localhost:5680/events" }),
  [int]$Count = $(if ($env:COUNT) { [int]$env:COUNT } else { 5 })
)

$ErrorActionPreference = "Stop"

function New-EventId {
  [guid]::NewGuid().ToString()
}

function New-Timestamp {
  (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
}

function Send-ReplayEvent {
  param([hashtable]$Payload)
  try {
    $json = $Payload | ConvertTo-Json -Depth 8 -Compress
    Invoke-WebRequest -Uri $ApiUrl -Method POST -ContentType "application/json" -Body $json -UseBasicParsing | Out-Null
  } catch {}
}

Write-Host "[replay] sending app switch flow..."
for ($i = 1; $i -le $Count; $i++) {
  Send-ReplayEvent @{
    schema_version = "1.0"
    event_id = New-EventId
    ts = New-Timestamp
    source = "replay"
    app = "Slack"
    event_type = "app_switch"
    priority = "P2"
    resource = @{ type = "app"; id = "Slack" }
    payload = @{ app = "Slack"; window_title = "Inbox"; browser_url = "https://mail.google.com" }
  }
  Send-ReplayEvent @{
    schema_version = "1.0"
    event_id = New-EventId
    ts = New-Timestamp
    source = "replay"
    app = "Chrome"
    event_type = "app_switch"
    priority = "P2"
    resource = @{ type = "app"; id = "Chrome" }
    payload = @{ app = "Chrome"; window_title = "Docs"; browser_url = "https://docs.google.com" }
  }
}

Write-Host "[replay] sending file activity..."
for ($i = 1; $i -le $Count; $i++) {
  Send-ReplayEvent @{
    schema_version = "1.0"
    event_id = New-EventId
    ts = New-Timestamp
    source = "replay"
    app = "Explorer"
    event_type = "file_created"
    priority = "P2"
    resource = @{ type = "file"; id = "C:\Users\test\Downloads\report$($i).pdf" }
    payload = @{ path = "C:\Users\test\Downloads\report$($i).pdf"; filename = "report$($i).pdf" }
  }
}

Write-Host "[replay] sending keyword activity..."
for ($i = 1; $i -le $Count; $i++) {
  Send-ReplayEvent @{
    schema_version = "1.0"
    event_id = New-EventId
    ts = New-Timestamp
    source = "replay"
    app = "Mail"
    event_type = "key_input"
    priority = "P2"
    resource = @{ type = "input"; id = "keyboard" }
    payload = @{ text = "invoice follow-up" }
  }
}

Write-Host "[replay] done. (API_URL=$ApiUrl)"
