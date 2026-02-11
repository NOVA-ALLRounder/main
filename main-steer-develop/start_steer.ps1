param(
    [switch]$Dev,
    [switch]$SkipDcp,
    [switch]$RunOnly
)
$ErrorActionPreference = "Stop"
Write-Host "Starting Steer (Tauri-first mode)..."
$env:STEER_ALLOW_MULTI = "1"
$env:STEER_API_PORT = "5680"
$env:STEER_DISABLE_ANALYZER = "1"
$env:STEER_DISABLE_BACKGROUND_ANALYSIS = "1"
if (-not $env:STEER_OPERATION_MODE) {
    $env:STEER_OPERATION_MODE = "autopilot"
}

$RepoRoot = $PSScriptRoot
$CoreDir = Join-Path $RepoRoot "core"
$env:STEER_HOME = Join-Path $CoreDir ".steer"
if (-not (Test-Path $env:STEER_HOME)) {
    New-Item -ItemType Directory -Force -Path $env:STEER_HOME | Out-Null
}

function Sync-EnvFileNoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path $Source)) {
        return $false
    }

    $txt = [System.IO.File]::ReadAllText($Source)
    if ($txt.Length -gt 0 -and $txt[0] -eq [char]0xFEFF) {
        $txt = $txt.Substring(1)
    }
    $enc = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Destination, $txt, $enc)
    return $true
}

function Stop-SteeerProcesses {
    $targets = @("app", "core", "local_os_agent")
    foreach ($name in $targets) {
        $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
        foreach ($p in $procs) {
            try {
                Stop-Process -Id $p.Id -Force -ErrorAction Stop
                Write-Host "Stopped process: $($p.ProcessName) ($($p.Id))"
            } catch {
                Write-Warning "Failed to stop process $($p.ProcessName) ($($p.Id)): $($_.Exception.Message)"
            }
        }
    }
}

function Clear-StaleSteerLock {
    $lockPath = Join-Path $env:LOCALAPPDATA "steer\steer.lock"
    if (-not (Test-Path $lockPath)) {
        return
    }

    try {
        $raw = Get-Content -Path $lockPath -Raw -ErrorAction Stop
        $data = $raw | ConvertFrom-Json -ErrorAction Stop
        $lockPid = [int]$data.pid
        $proc = Get-Process -Id $lockPid -ErrorAction SilentlyContinue
        if ($null -eq $proc) {
            Remove-Item -Path $lockPath -Force -ErrorAction Stop
            Write-Host "Removed stale steer lock (dead pid: $lockPid)"
        } else {
            Write-Host "Steer lock belongs to live pid: $lockPid"
        }
    } catch {
        try {
            Remove-Item -Path $lockPath -Force -ErrorAction Stop
            Write-Warning "Removed unreadable steer lock file: $lockPath"
        } catch {
            Write-Warning "Failed to clear steer lock: $($_.Exception.Message)"
        }
    }
}

function Wait-ApiHealthy {
    param(
        [string]$Url = "http://127.0.0.1:5680/api/system/health",
        [int]$TimeoutSeconds = 20
    )

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        try {
            $resp = Invoke-WebRequest -UseBasicParsing -Method Get -Uri $Url -TimeoutSec 2
            if ($resp -and $resp.StatusCode -eq 200) {
                return $true
            }
        } catch {
            # retry until timeout
        }
        Start-Sleep -Milliseconds 600
    }
    return $false
}

function Ensure-CoreApi {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseDir,
        [int]$TimeoutSeconds = 45
    )

    if (Wait-ApiHealthy -TimeoutSeconds $TimeoutSeconds) {
        return $true
    }

    $coreExe = Join-Path $ReleaseDir "core.exe"
    if (Test-Path $coreExe) {
        # Prevent duplicate core instances: app.exe may already be starting sidecar.
        $existingCore = @(Get-Process -Name core, local_os_agent -ErrorAction SilentlyContinue)
        if ($existingCore.Count -gt 0) {
            Write-Warning "API is not healthy yet, but core process already exists. Skipping fallback launch to avoid port conflicts."
            if (Wait-ApiHealthy -TimeoutSeconds ([Math]::Max(20, [int]($TimeoutSeconds / 2)))) {
                return $true
            }
            return $false
        }

        # One more grace window before fallback to avoid racing sidecar startup.
        if (Wait-ApiHealthy -TimeoutSeconds 15) {
            return $true
        }

        # If port is already listening, do not spawn another core.
        $portBusy = $false
        try {
            $listen = Get-NetTCPConnection -LocalPort 5680 -State Listen -ErrorAction SilentlyContinue
            if ($listen) {
                $portBusy = $true
            }
        } catch {}
        if ($portBusy) {
            Write-Warning "Port 5680 is already listening. Skipping fallback launch to avoid duplicate binding."
            return (Wait-ApiHealthy -TimeoutSeconds 20)
        }

        Write-Warning "API is not healthy yet. Starting hidden core.exe fallback (--api --port 5680)..."
        $fallbackOut = Join-Path $ReleaseDir "core.fallback.out.log"
        $fallbackErr = Join-Path $ReleaseDir "core.fallback.err.log"
        Start-Process -FilePath $coreExe `
            -ArgumentList @("--api", "--port", "5680") `
            -WorkingDirectory $ReleaseDir `
            -WindowStyle Hidden `
            -RedirectStandardOutput $fallbackOut `
            -RedirectStandardError $fallbackErr | Out-Null
        Start-Sleep -Milliseconds 800
        if (Wait-ApiHealthy -TimeoutSeconds $TimeoutSeconds) {
            return $true
        }
    }

    return $false
}

$WebDir = Join-Path $RepoRoot "web"
$SrcTauriDir = Join-Path $WebDir "src-tauri"
$DcpRunScript = Join-Path (Join-Path $RepoRoot "scripts") "run_all.ps1"
$EnvSource = Join-Path $CoreDir ".env"

$CoreBuildOut = Join-Path $CoreDir "target\debug\local_os_agent.exe"
$SidecarDir = Join-Path $SrcTauriDir "binaries"
$SidecarExe = Join-Path $SidecarDir "core-x86_64-pc-windows-msvc.exe"
$CredSource = Join-Path $CoreDir "credentials.json"

if ($RunOnly) {
    Write-Host "RunOnly mode: skipping build and launching existing binary..."
    Stop-SteeerProcesses
    Clear-StaleSteerLock

    $releaseDir = Join-Path $SrcTauriDir "target\release"
    $appExe = Join-Path $releaseDir "app.exe"
    if (-not (Test-Path $appExe)) {
        $appExe = Get-ChildItem -Path $releaseDir -Filter "*.exe" -File |
            Where-Object { $_.Name -notlike "*core*" } |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $appExe -or -not (Test-Path $appExe)) {
        throw "RunOnly failed: built Tauri executable not found under $releaseDir"
    }

    $ReleaseEnv = Join-Path $releaseDir ".env"
    if (Sync-EnvFileNoBom -Source $EnvSource -Destination $ReleaseEnv) {
        Write-Host "RunOnly: synced .env to release runtime directory."
    } else {
        Write-Warning "RunOnly: .env not found in core. API keys may be missing at runtime."
    }

    $ReleaseCoreExe = Join-Path $releaseDir "core.exe"
    if (Test-Path $CoreBuildOut) {
        Copy-Item -Force $CoreBuildOut $ReleaseCoreExe
        Write-Host "RunOnly: synced core binary to release runtime directory."
    } elseif (-not (Test-Path $ReleaseCoreExe)) {
        Write-Warning "RunOnly: core.exe missing in release runtime directory."
    }
    if (Test-Path $CredSource) {
        Copy-Item -Force $CredSource (Join-Path $releaseDir "credentials.json")
        Write-Host "RunOnly: synced credentials.json to release runtime directory."
    }

    Write-Host "Launching (RunOnly): $appExe"
    Start-Process -FilePath $appExe | Out-Null
    if (Ensure-CoreApi -ReleaseDir $releaseDir -TimeoutSeconds 60) {
        Write-Host "Steer Tauri app launched (RunOnly). API is healthy."
    } else {
        Write-Warning "Steer app launched, but API health check did not pass within timeout."
        Write-Warning "Check runtime logs and verify core sidecar startup."
    }
    exit 0
}

if (-not $SkipDcp) {
    if (Test-Path $DcpRunScript) {
        Write-Host "Launching DCP..."
        Start-Process powershell -ArgumentList "-NoExit", "-File", $DcpRunScript | Out-Null
    } else {
        Write-Warning "DCP script not found: $DcpRunScript"
    }
}

if (-not $Dev) {
    Push-Location $WebDir
    if (-not (Test-Path "node_modules")) {
        Write-Host "Installing web dependencies..."
        npm install
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            throw "npm install failed."
        }
    }
    if (-not (Test-Path "dist\\index.html")) {
        Write-Host "web dist missing. Building web assets..."
        npm run build
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            throw "web build failed and dist is missing."
        }
    } else {
        Write-Host "Using existing web dist."
    }
    Pop-Location
}

Write-Host "Building Core Agent..."
Push-Location $CoreDir
cargo build
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    throw "Core build failed."
}
Pop-Location

if (-not (Test-Path $CoreBuildOut)) {
    throw "Core binary not found: $CoreBuildOut"
}

if (-not (Test-Path $SidecarDir)) {
    New-Item -ItemType Directory -Force -Path $SidecarDir | Out-Null
}
Copy-Item -Force $CoreBuildOut $SidecarExe

$EnvDest = Join-Path $SidecarDir ".env"
if (Sync-EnvFileNoBom -Source $EnvSource -Destination $EnvDest) {
    Write-Host "Synced .env to sidecar binaries."
} else {
    Write-Warning ".env not found in core. API keys may be missing at runtime."
}
if (Test-Path $CredSource) {
    Copy-Item -Force $CredSource (Join-Path $SidecarDir "credentials.json")
    Write-Host "Synced credentials.json to sidecar binaries."
}

Push-Location $WebDir
if (-not (Test-Path "node_modules")) {
    Write-Host "Installing web dependencies..."
    npm install
    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        throw "npm install failed."
    }
}

if ($Dev) {
    Write-Host "Launching Tauri DEV mode..."
    Stop-SteeerProcesses
    Clear-StaleSteerLock
    npm run tauri dev
    Pop-Location
    exit 0
}

Write-Host "Building and launching Tauri app (default)..."
Stop-SteeerProcesses
Clear-StaleSteerLock
npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    throw "tauri build failed."
}
Pop-Location

$releaseDir = Join-Path $SrcTauriDir "target\release"
$ReleaseEnv = Join-Path $releaseDir ".env"
if (Sync-EnvFileNoBom -Source $EnvSource -Destination $ReleaseEnv) {
    Write-Host "Synced .env to release runtime directory."
}

$ReleaseCoreExe = Join-Path $releaseDir "core.exe"
if (Test-Path $CoreBuildOut) {
    Copy-Item -Force $CoreBuildOut $ReleaseCoreExe
    Write-Host "Synced core binary to release runtime directory."
}
if (Test-Path $CredSource) {
    Copy-Item -Force $CredSource (Join-Path $releaseDir "credentials.json")
    Write-Host "Synced credentials.json to release runtime directory."
}

$appExe = Join-Path $releaseDir "app.exe"
if (-not (Test-Path $appExe)) {
    $appExe = Get-ChildItem -Path $releaseDir -Filter "*.exe" -File |
        Where-Object { $_.Name -notlike "*core*" } |
        Select-Object -First 1 -ExpandProperty FullName
}

if (-not $appExe -or -not (Test-Path $appExe)) {
    throw "Built Tauri executable not found under $releaseDir"
}

Write-Host "Launching: $appExe"
Start-Process -FilePath $appExe | Out-Null
if (Ensure-CoreApi -ReleaseDir $releaseDir -TimeoutSeconds 60) {
    Write-Host "Steer Tauri app launched successfully. API is healthy."
} else {
    Write-Warning "Steer app launched, but API health check did not pass within timeout."
}
Write-Host "Tip: for dev mode use: .\start_steer.ps1 -Dev"


