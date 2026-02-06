# Data-Collection-Projection

> **Intelligent User Behavior Collection & Automation System**
>
> 사용자의 PC 사용 행동을 실시간으로 수집하고, AI가 반복 작업을 자동화하는 Python 시스템.

---

## 1. 핵심 기능

| 기능 | 설명 | 상태 |
|:---|:---|:---|
| 자동 수집 | 모든 앱에서 UI 이벤트 수집 | ✅ |
| 개인정보 보호 | 민감 앱 제외, PII 마스킹 | ✅ |
| 데이터 경량화 | 5분 집약, 7일 보존 | ✅ |
| Windows 자동 시작 | 로그온 시 수집기 실행 | ✅ |
| 종료 시 요약 | 시스템 종료 신호 감지 | ✅ |
| 패턴 분석 | 시작 시 어제 데이터 분석 | ✅ |
| LLM 워크플로우 | 누적 패턴으로 자동화 추천 | ⏳ |

---

## 2. 빠른 시작

```powershell
# 1. 환경 설정
conda activate Osdata
pip install -r requirements.txt

# 2. 수집기 실행
cd c:\os_data\Data-Collection-Projection
$env:PYTHONPATH="src"; python -m collector.main

# 3. 자동 시작 등록 (관리자 권한)
schtasks /create /tn "DataCollector" /tr "c:\os_data\Data-Collection-Projection\start_collector.bat" /sc onlogon /rl highest
```

---

## 3. 하루 흐름도

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        🌅 아침 - 컴퓨터 시작                                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 🚀 수집기 자동 시작 (작업 스케줄러)                                            │
│                                                                              │
│   1. DB 연결 + PrivacyGuard 로드                                             │
│   2. 📊 어제까지 패턴 분석 → workflow_YYYY-MM-DD.json 생성                     │
│   3. Aggregator 시작 (5분마다 집약)                                           │
│   4. HTTP 서버 + 센서 시작                                                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 💼 업무 시간 (이벤트 자동 수집)                                                │
│                                                                              │
│   [매 0.5초] UI Automation → 포커스된 요소 캡처                               │
│   [실시간] Input Hook → 마우스 클릭/단축키                                    │
│   [실시간] Browser Extension → 웹 클릭/URL                                   │
│                           ↓                                                  │
│   [즉시] DB에 저장 (events 테이블)                                            │
│   [매 5분] Aggregator → 분 단위 요약 (minute_aggregates)                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ 🌙 퇴근 - 컴퓨터 종료                                                         │
│                                                                              │
│   Windows 종료 신호 감지 (win32api)                                           │
│              ↓                                                               │
│   1. 마지막 5분 데이터 집약                                                   │
│   2. 📋 오늘 일일 요약 생성 (daily_summaries)                                 │
│   3. 7일 이전 원시 이벤트 삭제                                                │
│   4. 수집기 정상 종료                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 종료 시 요약 생성

| 종료 방식 | 요약 생성 |
|:---|:---|
| Ctrl+C | ✅ |
| 시스템 종료 | ✅ |
| 재부팅 | ✅ |
| 로그오프 | ✅ |
| 강제 전원 OFF | ❌ (다음 시작 시 복구 예정) |

---

## 5. 생성되는 데이터

```
workflows/
├── workflow_2026-02-04.json   # 시작 시 어제 패턴 분석
├── workflow_2026-02-05.json
└── workflow_2026-02-06.json

collector.db (SQLite)
├── events              # 원시 이벤트 (7일 보존)
├── minute_aggregates   # 5분 요약 (30일 보존)
└── daily_summaries     # 일일 요약 (영구 보존)
```

### 예상 데이터량 (8시간 근무 기준)

| 기간 | 용량 |
|:---|:---|
| 하루 | ~3-5 MB |
| 일주일 | ~20-35 MB |
| 최대 (7일 유지) | ~50 MB |

---

## 6. 프로젝트 구조

```
Data-Collection-Projection/
├── src/collector/
│   ├── main.py              # 수집기 엔트리 (종료 신호 감지 포함)
│   ├── aggregator.py        # 5분 집약 + 종료 시 일일 요약
│   └── startup_workflow.py  # 시작 시 패턴 분석
├── src/sensors/os/
│   ├── ui_automation.py     # UI 요소 폴링 센서
│   └── input_hook.py        # 마우스/키보드 훅 센서
├── browser_extension/       # Chrome 확장
├── configs/privacy_rules.yaml
├── workflows/               # 자동 생성 워크플로우
├── start_collector.bat      # 자동 시작 배치
└── requirements.txt
```

---

## 7. 수집 범위

### 수집 대상
| 항목 | 수집 |
|:---|:---|
| 모든 앱 | ✅ (denylist 제외) |
| 버튼/메뉴 클릭 | ✅ |
| 마우스 클릭 좌표 | ✅ |
| 단축키 (Ctrl+S 등) | ✅ |
| 입력창 내용 | ✅ (100자 제한) |

### 자동 제외
- 비밀번호 관리자 (1Password, KeePass)
- 로그인/결제 창
- 이메일/전화번호 → 자동 마스킹

---

## 8. 관리 명령어

```powershell
# 수집기 수동 실행
$env:PYTHONPATH="src"; python -m collector.main

# 작업 스케줄러 제어
schtasks /run /tn "DataCollector"     # 시작
schtasks /end /tn "DataCollector"     # 중지
schtasks /delete /tn "DataCollector" /f  # 삭제

# LLM 워크플로우 생성 (수동)
python scripts/generate_workflow.py
```

---

## 9. 기술 스택

| 구분 | 기술 |
|:---|:---|
| 언어 | Python 3.11+ |
| UI 자동화 | uiautomation |
| 입력 훅 | pynput |
| Windows 이벤트 | pywin32 |
| DB | SQLite (WAL) |
| LLM | OpenAI GPT-4o-mini |

---

## 10. 구현 진행률

```
[████████████░░░░░░░░] 60%
```

| 완료 | 미완료 |
|:---|:---|
| 수집 인프라 | LLM 일일 분석 |
| DB 경량화 | 우선순위/중요도 파악 |
| 자동 시작/종료 | 누적 패턴 비교 |
| 종료 시 요약 생성 | 워크플로우 추천 |
| 빈도 기반 패턴 | 강제 종료 복구 |

---

*2026-02-06 기준 작성*
