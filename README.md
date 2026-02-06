# Local OS Agent (Rust Native)

**사용자 행동 기반 자동화 에이전트** - 컴퓨터 사용 패턴을 분석하여 자동화를 추천하고 실행합니다.

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust)](https://www.rust-lang.org/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple)](https://www.apple.com/macos/)

## ✨ 주요 기능

| 기능 | 명령어 | 설명 |
|:---|:---|:---|
| Shadow | (자동) | 백그라운드 행동 데이터 수집 |
| Routine | `routine` | 일일 루틴 분석 |
| Recommend | `recommend` | 자동화 스크립트 제안 |
| Control | `control <app> <cmd>` | 앱 내부 제어 |
| Workflow | `build_workflow <prompt>` | n8n 자동화 생성 |
| Exec | `exec <cmd>` | 셸 명령 실행 |
| Status | `status` | 시스템 리소스 확인 |

## 🆕 JARVIS - Advanced AI Agent System

JARVIS is our next-generation AI agent orchestration system with modular skills, parallel execution, and comprehensive automation capabilities.

### ✅ New Features

- **🤖 Modular Skill System**: Extensible architecture with plug-and-play skills
  - ComputerUseSkill: Complete computer control (keyboard, mouse, screen capture)
  - EmailSkill: Full email automation (SMTP/IMAP)
  - TelegramSkill: Telegram bot integration

- **⚡ Parallel Execution Engine**: 5x faster task execution with intelligent dependency resolution
  - Automatic detection of independent tasks
  - Smart scheduling for optimal performance
  - Resource-efficient concurrent execution

- **🧠 Intelligent Task Planning**: AI-powered task decomposition and orchestration
  - Complex workflow management
  - Multi-step task coordination
  - Error recovery and retry logic

- **🔌 Easy Skill Development**: Simple trait-based API for creating custom skills
  - Well-documented interfaces
  - Comprehensive testing support
  - Hot-reload capability

### 🚀 Performance

- **5x faster** with parallel execution for independent tasks
- Intelligent task scheduling and dependency resolution
- Async I/O throughout for maximum efficiency
- Resource pooling for external connections

### 🔧 Configuration

JARVIS uses environment variables for configuration. Create a `.env` file:

```bash
# OpenAI/LLM Configuration
OPENAI_API_KEY=sk-...

# Email Configuration (Optional)
SMTP_SERVER=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_FROM=your-email@gmail.com

IMAP_SERVER=imap.gmail.com
IMAP_PORT=993
IMAP_USERNAME=your-email@gmail.com
IMAP_PASSWORD=your-app-password

# Telegram Configuration (Optional)
TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjklMNOpqrsTUVwxyz
```

### 📚 Documentation

- **[Skills Documentation](docs/SKILLS.md)**: Comprehensive guide to all skills, actions, and parameters
- **[API Reference](docs/API.md)**: Complete API documentation
- **[Development Guide](docs/DEVELOPMENT.md)**: Guide for creating custom skills

### 🧪 Testing

Run the comprehensive test suite:

```bash
# Run all tests
cd core
cargo test

# Run integration tests only
cargo test --test jarvis_integration_test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_full_workflow
```

Test coverage includes:
- Unit tests for each skill
- Integration tests for orchestrator
- End-to-end workflow tests
- Error handling and recovery tests
- Performance benchmarks

## 🚀 설치

```bash
# 1. Clone
git clone <repo_url>
cd local-os-agent/core

# 2. 환경변수 설정
cp .env.example .env
# .env 파일에 OPENAI_API_KEY 입력

# 3. 빌드
cargo build --release

# 4. 실행 (Accessibility 권한 필요)
./target/release/core
```

## 📦 Release

To build a production-ready application (binary/bundle):

```bash
./scripts/build_release.sh
```

This script automates:
1.  **Frontend Build**: Compiles React/Vite assets.
2.  **Core Build**: Compiles Rust sidecar (steer-core).
3.  **Bundle**: Generates `.app` (macOS) or `.exe` in `desktop/src-tauri/target/release/bundle`.

## 🛡️ Self-Healing
The agent includes a supervisor script to ensure high availability:

```bash
./scripts/steer-guardian.sh
```
This restarts the core process automatically if a crash occurs.

## 📋 필수 요구사항

- **macOS 12+** (Monterey 이상)
- **Accessibility 권한**: 시스템 환경설정 → 개인정보 보호 → 손쉬운 사용 → 터미널 체크
- **Rust 1.70+**
- **OpenAI API Key** (LLM 분석용)

## 🛡️ 보안

- `exec` 명령어는 위험한 키워드(`rm`, `sudo` 등)가 포함되면 차단됩니다.
- 기본적으로 **Write Lock**이 활성화되어 있습니다. `unlock` 명령어로 해제하세요.

## 📂 프로젝트 구조

```
core/src/
├── main.rs          # CLI 및 메인 루프
├── analyzer.rs      # 행동 패턴 분석기
├── db.rs            # SQLite 저장소
├── policy.rs        # 보안 정책 엔진
├── executor.rs      # 셸 명령 실행
├── llm_gateway.rs   # OpenAI 연동
├── notifier.rs      # macOS 알림
├── monitor.rs       # 시스템 모니터링
├── applescript.rs   # 앱 제어
├── n8n_api.rs       # n8n 워크플로우 API
├── visual_driver.rs # UI 자동화 폴백
└── macos/           # 네이티브 macOS 바인딩
```

## 🧪 테스트

```bash
cargo test
```

## 📜 라이선스

MIT License
