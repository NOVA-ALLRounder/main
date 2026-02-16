param(
    [string]$ApiUrl = "http://localhost:5680/events",
    [int]$Count = 5
)

function New-UtcTimestamp {
    return (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
}

function New-EventId {
    return [guid]::NewGuid().ToString()
}

function Send-Event($payload) {
    try {
        Invoke-RestMethod -Uri $ApiUrl -Method Post -ContentType "application/json" -Body ($payload | ConvertTo-Json -Depth 6) | Out-Null
    } catch {
        # Best-effort; ignore transient errors
    }
}

Write-Host "[replay] sending app switch flow..."
for ($i = 1; $i -le $Count; $i++) {
    $ts = New-UtcTimestamp
    Send-Event @{ 
        schema_version = "1.0"; event_id = (New-EventId); ts = $ts; source = "replay"; app = "Slack";
        event_type = "app_switch"; priority = "P2";
        resource = @{ type = "app"; id = "Slack" };
        payload = @{ app = "Slack"; window_title = "Inbox"; browser_url = "https://mail.google.com" }
    }
    Send-Event @{ 
        schema_version = "1.0"; event_id = (New-EventId); ts = $ts; source = "replay"; app = "Chrome";
        event_type = "app_switch"; priority = "P2";
        resource = @{ type = "app"; id = "Chrome" };
        payload = @{ app = "Chrome"; window_title = "Docs"; browser_url = "https://docs.google.com" }
    }
}

Write-Host "[replay] sending file activity..."
$downloads = Join-Path $env:USERPROFILE "Downloads"
for ($i = 1; $i -le $Count; $i++) {
    $path = Join-Path $downloads ("report{0}.pdf" -f $i)
    Send-Event @{ 
        schema_version = "1.0"; event_id = (New-EventId); ts = (New-UtcTimestamp); source = "replay"; app = "Explorer";
        event_type = "file_created"; priority = "P2";
        resource = @{ type = "file"; id = $path };
        payload = @{ path = $path; filename = ("report{0}.pdf" -f $i) }
    }
}

Write-Host "[replay] sending keyword activity..."
for ($i = 1; $i -le $Count; $i++) {
    Send-Event @{ 
        schema_version = "1.0"; event_id = (New-EventId); ts = (New-UtcTimestamp); source = "replay"; app = "Mail";
        event_type = "key_input"; priority = "P2";
        resource = @{ type = "input"; id = "keyboard" };
        payload = @{ text = "invoice follow-up" }
    }
}

Write-Host "[replay] done. (API_URL=$ApiUrl)"
