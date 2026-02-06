# Data-Collection-Projection

> **Intelligent User Behavior Collection & Automation System**
>
> 사용자의 PC 사용 행동을 실시간으로 수집하고, AI가 반복 작업을 자동화하는 Python 시스템.

---

## 1. 프로젝트 개요

### 1.1 목표
Windows PC에서 사용자의 모든 행동(앱 전환, 버튼 클릭, 텍스트 입력 등)을 자동 수집하고, 반복 패턴을 감지하여 자동화 워크플로우를 생성.

### 1.2 핵심 가치
| 기존 매크로 | 본 시스템 |
| :--- | :--- |
| 좌표 기반 클릭 | **요소 기반** ("저장 버튼 클릭") |
| 창 크기 바뀌면 실패 | 창 위치 무관 동작 |
| 사람이 매크로 작성 | **AI가 패턴 자동 감지** |

---

## 2. 빠른 시작

### 2.1 설치
```powershell
# 1. Conda 환경 활성화
conda activate Osdata

# 2. 의존성 설치
pip install -r requirements.txt

# 3. API 키 설정 (.env 파일)
echo "LLM_API_KEY=sk-your-openai-key" > .env
```

### 2.2 수집기 실행
```powershell
cd c:\os_data\Data-Collection-Projection
$env:PYTHONPATH="src"; python -m collector.main
```

### 2.3 자동 시작 설정 (Windows 로그온 시)
```powershell
# 관리자 권한 PowerShell에서 실행
schtasks /create /tn "DataCollector" /tr "c:\os_data\Data-Collection-Projection\start_collector.bat" /sc onlogon /rl highest
```

---

## 3. 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────────────────┐
│                      사용자 PC (Windows)                              │
│                                                                      │
│  ┌─────────────── Sensors ───────────────┐                          │
│  │  UI Automation  │  Input Hook  │ Browser Extension │             │
│  │   (0.5초 폴링)   │ (실시간 훅)   │    (DOM 이벤트)    │             │
│  └───────────────────────┬───────────────────────────┘             │
│                          │ HTTP POST /events                        │
│                          ▼                                          │
│  ┌─────────────────────────────────────────────────────────┐       │
│  │                      Collector                           │       │
│  │  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │       │
│  │  │PrivacyGuard │→│ EventStore   │→│  Aggregator   │  │       │
│  │  │(필터+마스킹) │  │ (SQLite)     │  │ (5분 자동집약) │  │       │
│  │  └─────────────┘  └──────────────┘  └───────────────┘  │       │
│  └─────────────────────────────────────────────────────────┘       │
│                          │                                          │
│            ┌─────────────┴─────────────┐                           │
│            ▼                           ▼                           │
│  ┌──────────────────┐       ┌──────────────────┐                   │
│  │ Startup Workflow │       │  Daily Summary   │                   │
│  │   (시작시 생성)   │       │   (종료시 생성)   │                   │
│  └──────────────────┘       └──────────────────┘                   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. 자동화 흐름

### 4.1 하루 전체 흐름

```
🖥️ 09:00  컴퓨터 부팅 + 로그인
     │
     ▼
🚀 수집기 자동 시작 (작업 스케줄러)
     │
     ├── 📊 어제까지 패턴 분석 → workflow_2026-02-05.json 생성
     ├── HTTP 서버 + 센서 시작
     └── 5분마다 자동 집약 시작
     │
     ▼
💼 09:00~18:00  사용자 작업 (이벤트 자동 수집)
     │
     ▼
🛑 수집기 종료 (Ctrl+C 또는 컴퓨터 종료)
     │
     ├── 마지막 5분 데이터 집약
     ├── 오늘 일일 요약 생성 (daily_summaries)
     └── 7일 이전 원시 이벤트 삭제
```

### 4.2 생성되는 파일

```
workflows/
├── workflow_2026-02-04.json   # 2월 5일 시작 시 생성
├── workflow_2026-02-05.json   # 2월 6일 시작 시 생성
└── workflow_2026-02-06.json   # 2월 7일 시작 시 생성

collector.db (SQLite)
├── events              # 원시 이벤트 (7일 보존)
├── minute_aggregates   # 5분 단위 요약 (30일 보존)
└── daily_summaries     # 일일 요약 (영구 보존)
```

---

## 5. 프로젝트 구조

```
Data-Collection-Projection/
├── src/
│   ├── collector/
│   │   ├── main.py              # 수집기 엔트리포인트
│   │   ├── aggregator.py        # 5분 자동 집약 + 종료 시 일일 요약
│   │   └── startup_workflow.py  # 시작 시 패턴 분석 + 워크플로우 생성
│   └── sensors/
│       └── os/
│           ├── ui_automation.py # UI 요소 폴링 센서
│           ├── input_hook.py    # 마우스/키보드 훅 센서
│           └── emit.py          # HTTP 전송 유틸리티
├── browser_extension/           # Chrome 확장 (DOM 이벤트)
├── configs/
│   └── privacy_rules.yaml      # 수집 규칙 (전체 수집 모드)
├── workflows/                   # 자동 생성된 워크플로우 (날짜별)
├── scripts/
│   ├── generate_workflow.py    # LLM 코드 생성 (수동)
│   ├── build_aggregation.py    # 수동 집약 스크립트
│   └── data_compressor.py      # 데이터 압축/정리
├── start_collector.bat          # 자동 시작용 배치 파일
├── collector.db                 # 수집 데이터 (SQLite)
└── requirements.txt
```

---

## 6. 수집 범위

### 6.1 현재 설정: **전체 수집 모드**

| 항목 | 수집 | 비고 |
| :--- | :--- | :--- |
| 모든 앱 | ✅ | denylist 제외 |
| 버튼/메뉴 클릭 | ✅ | 요소 이름 포함 |
| 마우스 좌표 | ✅ | 클릭 위치 |
| 단축키 | ✅ | Ctrl+S 등 |
| 입력창 내용 | ✅ | 100자 제한 |
| 웹 페이지 클릭 | ✅ | Chrome 확장 필요 |

### 6.2 자동 제외
- 비밀번호 관리자 (1Password, KeePass 등)
- 로그인/결제 창
- 이메일/전화번호 자동 마스킹

---

## 7. 데이터 경량화

### 7.1 자동 집약
```
원시 이벤트 (0.5초마다)
     ↓ 5분마다
분 단위 요약 (minute_aggregates)
     ↓ 수집기 종료 시
일일 요약 (daily_summaries)
     ↓ 7일 후
원시 이벤트 자동 삭제
```

### 7.2 예상 데이터량
- **일주일**: ~50-70MB
- **요약 데이터**: ~5MB/월

---

## 8. 관리 명령어(POWERSHELL)

```powershell
# 수집기 수동 실행
$env:PYTHONPATH="src"; python -m collector.main

# 수집기 자동 시작 (작업 스케줄러)
schtasks /run /tn "DataCollector"

# 수집기 중지
schtasks /end /tn "DataCollector"

# 작업 스케줄러에서 제거
schtasks /delete /tn "DataCollector" /f

# 수동 집약 실행
python scripts/build_aggregation.py --date 2026-02-06

# LLM으로 자동화 코드 생성 (수동)
python scripts/generate_workflow.py
```

---

## 9. 설정 파일

### 9.1 `configs/privacy_rules.yaml`
```yaml
# 전체 수집 (allowlist 비어있음)
allowlist_apps: []

# 민감 앱 제외
denylist_apps:
  - 1Password.exe
  - KeePass.exe

# 민감 창 제목 패턴
sensitive_window_patterns:
  - "비밀번호"
  - "로그인"
  - "결제"
```

---

## 10. 기술 스택

| 구분 | 기술 |
| :--- | :--- |
| 언어 | Python 3.11+ |
| UI 자동화 | `uiautomation` |
| 입력 훅 | `pynput` |
| 데이터베이스 | SQLite (WAL 모드) |
| LLM | OpenAI GPT-4o-mini |
| 설정 파일 | YAML |

---

*이 문서는 2026-02-06 기준으로 작성되었습니다.*
