# 🚀 Steer - 빠른 시작 가이드

## 한 방에 실행하기

### 방법 1: 통합 스크립트 (추천) ⭐

**Windows 배치 파일 실행:**
```bash
start.bat
```

**또는 PowerShell 직접 실행:**
```powershell
.\start.ps1
```

이 스크립트는 자동으로:
1. ✅ 백엔드(Rust) 빌드 확인
2. 🚀 백엔드 API 서버 시작 (http://localhost:8080)
3. 🎨 Tauri 데스크톱 앱 실행
4. 🔗 자동 연결

---

### 방법 2: 개별 실행

#### 백엔드만 실행
```bash
cd core
cargo run --bin local_os_agent
```

#### 프론트엔드만 실행
```bash
cd web
npm run tauri:dev
```

---

## 📦 첫 실행 전 준비

### 1. 의존성 설치

**백엔드 (Rust):**
```bash
cd core
cargo build
```

**프론트엔드 (Node.js):**
```bash
cd web
npm install
```

### 2. 환경 설정 (선택사항)

**`.env` 파일 생성** (core 디렉토리):
```env
# API Keys (선택)
ANTHROPIC_API_KEY=your_claude_api_key
TELEGRAM_BOT_TOKEN=your_telegram_token

# 서버 설정
API_PORT=8080
```

---

## 🎮 사용 방법

### JARVIS 명령어 사용

1. **데스크톱 앱 실행 후**
2. **JARVIS 탭 선택**
3. **명령어 입력:**

```
메모장 열어
크롬 실행해
클릭 100 200
스크린샷 찍어줘
```

### 또는 CLI로 직접:

```bash
cd core
cargo run --bin local_os_agent

> jarvis
JARVIS> 메모장 열어
✅ Notepad launched
```

---

## 🔧 문제 해결

### 포트 충돌
```bash
# 8080 포트 이미 사용 중인 경우
netstat -ano | findstr :8080
taskkill /PID <프로세스ID> /F
```

### 빌드 에러
```bash
# Rust 업데이트
rustup update

# 캐시 정리
cd core
cargo clean
cargo build
```

### Node 모듈 에러
```bash
cd web
rm -rf node_modules package-lock.json
npm install
```

---

## 📊 시스템 확인

### 백엔드 상태 확인
```bash
curl http://localhost:8080/health
```

### 프론트엔드 개발 서버 (Vite only)
```bash
cd web
npm run dev
# http://localhost:1420
```

---

## 🎯 다음 단계

1. **JARVIS_PROJECT_SUMMARY.md** - 전체 기능 문서
2. **ARCHITECTURE.md** - 시스템 아키텍처
3. **web/README.md** - 프론트엔드 가이드
4. **core/README.md** - 백엔드 API 문서

---

## 🆘 도움말

- **로그 확인**: `core/logs/` 디렉토리
- **데이터베이스**: `core/steer.db` (SQLite)
- **설정 파일**: `core/.env`

---

**즐거운 자동화 되세요!** 🎉
