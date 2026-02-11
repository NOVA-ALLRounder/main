# Steer - 통합 실행 스크립트
# Backend (Rust) + Frontend (Tauri) 동시 실행

Write-Host "🚀 Steer 시스템 시작 중..." -ForegroundColor Cyan
Write-Host ""

# 백엔드 빌드 확인
Write-Host "📦 백엔드 빌드 확인 중..." -ForegroundColor Yellow
Push-Location core
$buildResult = cargo build --bin local_os_agent 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ 백엔드 빌드 실패!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Write-Host "✅ 백엔드 빌드 완료" -ForegroundColor Green
Pop-Location

Write-Host ""
Write-Host "🎯 시스템 실행 중..." -ForegroundColor Cyan
Write-Host "   - Backend: http://localhost:8080" -ForegroundColor Gray
Write-Host "   - Frontend: Tauri Desktop App" -ForegroundColor Gray
Write-Host ""
Write-Host "⚠️  종료하려면 Ctrl+C 누르세요" -ForegroundColor Yellow
Write-Host ""

# 백엔드 실행 (백그라운드)
Write-Host "🔧 백엔드 시작 중..." -ForegroundColor Magenta
$backend = Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD\core'; cargo run --bin local_os_agent" -PassThru -WindowStyle Normal

# 2초 대기 (백엔드 초기화)
Start-Sleep -Seconds 2

# 프론트엔드 실행
Write-Host "🎨 프론트엔드 시작 중..." -ForegroundColor Magenta
Push-Location web
npm run tauri dev

# 프론트엔드 종료 시 백엔드도 종료
Pop-Location
Write-Host ""
Write-Host "🛑 프론트엔드 종료됨. 백엔드도 종료 중..." -ForegroundColor Yellow
Stop-Process -Id $backend.Id -Force -ErrorAction SilentlyContinue
Write-Host "✅ 모든 프로세스 종료 완료" -ForegroundColor Green
