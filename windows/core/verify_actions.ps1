param()

$ErrorActionPreference = "Stop"

Write-Host "Running system verification script..."
Write-Host "1. [TEST] Opening Notepad..."
Start-Process notepad.exe
Start-Sleep -Seconds 2
Write-Host "2. [TEST] Opening https://www.google.com ..."
Start-Process "https://www.google.com"
Write-Host "Verification complete. If you saw Notepad and Google, your system is fine."
