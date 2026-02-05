# Data-Collection-Projection: 프로젝트 전체 문서

> **Intelligent User Behavior Automation System**
>
> 사용자의 PC 사용 행동을 고해상도로 수집하고, AI(LLM)를 통해 반복 작업을 자동화하는 파이썬 기반 시스템.

---

## 1. 프로젝트 개요

### 1.1 목표
사용자가 Windows PC에서 수행하는 행동(앱 전환, 버튼 클릭, 텍스트 입력 등)을 실시간으로 수집하고, 이 데이터를 기반으로 LLM(GPT-4o-mini)이 자동화 Python 스크립트를 생성해 주는 시스템.

### 1.2 핵심 가치
| 기존 매크로 방식 | 본 시스템 방식 |
| :--- | :--- |
| 좌표 기반 (x:100, y:200 클릭) | **요소 기반** ("저장 버튼 클릭") |
| 창 크기/위치 바뀌면 실패 | 창 크기/위치 무관하게 동작 |
| 사람이 매크로 직접 작성 | **AI가 코드 자동 생성** |

### 1.3 주요 사용 시나리오
1.  사용자가 매일 아침 Outlook에서 메일을 쓰고, Excel에서 값을 입력하는 반복 작업을 수행.
2.  시스템이 이 행동을 자동으로 기록.
3.  "이 작업을 자동화해줘"라고 요청하면, AI가 `pywinauto` 또는 `win32com` 기반의 Python 코드를 생성.
4.  다음부터는 해당 코드를 실행하면 작업이 자동으로 수행됨.

---

## 2. 시스템 아키텍처

```
┌──────────────────────────────────────────────────────────────────────────┐
│                           사용자 PC (Windows)                              │
│                                                                           │
│   ┌─────────────────────────── Sensors ───────────────────────────┐      │
│   │                                                                │      │
│   │  ┌──────────────┐  ┌──────────────┐  ┌───────────────────┐   │      │
│   │  │ UI Automation│  │  Input Hook  │  │ Browser Extension │   │      │
│   │  │  (0.5초 폴링) │  │ (실시간 훅)  │  │  (DOM 이벤트)     │   │      │
│   │  └──────┬───────┘  └──────┬───────┘  └─────────┬─────────┘   │      │
│   │         │                  │                    │              │      │
│   │         └──────────────────┼────────────────────┘              │      │
│   │                            │ HTTP POST /events                 │      │
│   └────────────────────────────┼───────────────────────────────────┘      │
│                                ▼                                          │
│                   ┌─────────────────────────────┐                         │
│                   │         Collector           │                         │
│                   │    (HTTP Ingest Server)     │                         │
│                   │                             │                         │
│                   │  ┌───────────────────────┐  │                         │
│                   │  │      EventBus         │  │                         │
│                   │  │  (Queue + EventMerger)│  │                         │
│                   │  └───────────┬───────────┘  │                         │
│                   │              │              │                         │
│                   │              ▼              │                         │
│                   │  ┌───────────────────────┐  │                         │
│                   │  │     SQLite Store      │  │                         │
│                   │  │    (collector.db)     │  │                         │
│                   │  └───────────────────────┘  │                         │
│                   └─────────────────────────────┘                         │
│                                │                                          │
│                                ▼                                          │
│                   ┌─────────────────────────────┐                         │
│                   │      Pattern Analyzer       │                         │
│                   │   (analyze_patterns.py)     │                         │
│                   └──────────────┬──────────────┘                         │
│                                  │                                        │
│                                  ▼                                        │
│                   ┌─────────────────────────────┐                         │
│                   │    Workflow Generator       │                         │
│                   │  (generate_workflow.py)     │                         │
│                   │     ↓ LLM (GPT-4o-mini)     │                         │
│                   └──────────────┬──────────────┘                         │
│                                  │                                        │
│                                  ▼                                        │
│                   ┌─────────────────────────────┐                         │
│                   │   generated_workflow.py     │                         │
│                   │     (자동화 코드 출력)       │                         │
│                   └─────────────────────────────┘                         │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 기술 스택

### 3.1 언어 및 런타임
| 구분 | 기술 |
| :--- | :--- |
| 언어 | **Python 3.11+** |
| 패키지 관리 | Conda (Osdata 환경) / pip |

### 3.2 핵심 라이브러리
| 라이브러리 | 용도 |
| :--- | :--- |
| `uiautomation` | Windows UI Automation API 접근, UI 요소(버튼, 입력창 등) 정보 추출 |
| `pynput` | 키보드/마우스 이벤트 실시간 캡처 (Input Hook 센서) |
| `openai` | OpenAI GPT API 호출 (코드 생성) |
| `python-dotenv` | `.env` 파일에서 API 키 등 환경 변수 로드 |
| `PyYAML` | YAML 설정 파일 파싱 (`config.yaml`, `privacy_rules.yaml`) |
| `cryptography` | 데이터 암호화 (선택적 기능) |

### 3.3 데이터 저장
| 구분 | 기술 |
| :--- | :--- |
| 데이터베이스 | **SQLite** (`collector.db`) |
| 모드 | WAL (Write-Ahead Logging) |

### 3.4 외부 API
| 서비스 | 용도 |
| :--- | :--- |
| OpenAI API (`gpt-4o-mini`) | 수집된 로그를 자동화 Python 코드로 변환 |

---

## 4. 프로젝트 구조

```
Data-Collection-Projection/
├── .env                          # API 키 등 환경 변수 (Git 제외)
├── configs/
│   ├── config.yaml               # 수집기 설정 (포트, 센서 간격 등)
│   └── privacy_rules.yaml        # 허용/차단 앱 목록, PII 마스킹 규칙
├── migrations/                   # DB 스키마 마이그레이션 SQL
├── scripts/
│   ├── analyze_patterns.py       # 세션별 패턴 시각화
│   ├── generate_workflow.py      # LLM 코드 생성 스크립트
│   └── seed_dummy_data.py        # 테스트용 더미 데이터 삽입
├── src/
│   ├── collector/                # 수집기 핵심 모듈
│   │   ├── main.py               # 수집기 엔트리포인트
│   │   ├── bus.py                # 이벤트 큐 및 정규화
│   │   ├── store.py              # SQLite 저장 로직
│   │   ├── config.py             # 설정 클래스
│   │   ├── models.py             # 데이터 모델 (EventEnvelope 등)
│   │   ├── sessionizer.py        # 세션 분리 로직
│   │   ├── features.py           # 세션 요약 생성
│   │   └── event_merger.py       # 이벤트 병합기 (다중 센서 상관관계)
│   └── sensors/
│       └── os/
│           ├── ui_automation.py  # UI Automation 센서 (폴링 방식)
│           ├── input_hook.py     # Input Hook 센서 (실시간 이벤트 훅)
│           ├── emit.py           # HTTP 전송 유틸리티
│           └── windows_foreground.py # 포그라운드 창 변경 감지
├── browser_extension/            # Chrome 브라우저 확장
│   ├── manifest.json             # 확장 설정
│   ├── background.js             # 탭 변경/URL 캡처
│   └── content.js                # DOM 클릭/폼 제출 캡처
├── tests/                        # 테스트 코드
├── logs/                         # 수집기 로그 파일
├── collector.db                  # 수집된 이벤트 저장 DB
├── generated_workflow.py         # LLM이 생성한 자동화 코드 (출력물)
└── requirements.txt              # 의존성 목록
```

---

## 5. 실행 방법

### 5.1 사전 준비
1.  **Conda 환경 활성화**:
    ```powershell
    conda activate Osdata
    ```
2.  **의존성 설치**:
    ```powershell
    pip install -r requirements.txt
    ```
3.  **API 키 설정** (`.env` 파일 편집):
    ```
    LLM_API_KEY=sk-여기에_실제_OpenAI_키_입력
    ```

### 5.2 수집기 실행
```powershell
$env:PYTHONPATH="src"; python -m collector.main
```
- 터미널에 실시간 로그가 출력됩니다: `[Excel - Report.xlsx] Button '저장'`
- 종료: `Ctrl+C`

### 5.3 자동화 코드 생성
```powershell
python scripts/generate_workflow.py
```
- 최근 세션 로그를 GPT에게 보내고, `generated_workflow.py` 파일을 생성합니다.

### 5.4 패턴 분석 (선택)
```powershell
python scripts/analyze_patterns.py --hours 1
```
- 지난 1시간 동안의 세션을 타임라인으로 출력합니다.

---

## 6. 핵심 동작 흐름

### 6.1 데이터 수집 (Collector)
```
[1] UIAutomationSensor.capture_and_send()
    ├── uiautomation.GetFocusedControl()  → 현재 포커스된 UI 요소 가져오기
    ├── 요소 정보 수집:
    │   - ControlTypeName (Button, Edit, Pane 등)
    │   - Name ("저장", "검색" 등)
    │   - Value (입력된 텍스트)
    │   - BoundingRectangle (좌표)
    │   - WindowTitle (창 제목)
    ├── 해시 기반 중복 제거 (동일 상태면 SKIP)
    └── HttpEmitter.send_event() → POST http://127.0.0.1:8080/events

[2] Collector (main.py)
    ├── IngestHandler.do_POST()  → 이벤트 수신
    ├── EventBus.enqueue()       → 큐에 추가
    ├── PrivacyGuard.apply()     → 허용된 앱인지 확인 (allowlist_apps)
    │                             → PII 마스킹 (이메일, 전화번호)
    └── SQLiteStore.insert_events() → DB 저장
```

### 6.2 Input Hook 센서
```
[1] InputHookSensor (pynput 기반)
    ├── mouse.Listener           → 마우스 클릭/스크롤 감지
    ├── keyboard.Listener        → 키보드 이벤트 감지
    ├── 캡처 항목:
    │   - 마우스 클릭 (좌/우/중 버튼 + 좌표)
    │   - 스크롤 (방향 + 델타)
    │   - 특수 키 (Enter, Tab, F1-F12 등)
    │   - 단축키 (Ctrl+S, Ctrl+C 등)
    ├── 제외: 일반 문자 키 (개인정보 보호)
    └── 100ms 디바운스로 노이즈 필터링
```

### 6.3 브라우저 확장 (Browser Extension)
```
[1] content.js
    ├── document.click 이벤트    → DOM 요소 클릭 캡처
    │   - 태그, ID, 클래스, 텍스트, aria-label
    │   - 요소 목적 파악 (button, link, input)
    ├── document.submit 이벤트   → 폼 제출 캡처
    └── 300ms 디바운스

[2] background.js
    ├── tabs.onActivated         → 탭 전환 감지
    ├── tabs.onUpdated           → URL/타이틀 변경 감지
    └── 페이지 콘텐츠 요약 수집
```

### 6.4 패턴 분석 (Analyzer)
```
[1] analyze_patterns.py
    ├── fetch_events()         → DB에서 최근 이벤트 조회
    ├── group_by_session()     → 15분 이상 갭이 있으면 세션 분리
    └── print_session_timeline() → 터미널에 출력
        예: "[07:17:20] [Outlook.exe] [Button] 'New Email'"
```

### 6.3 코드 생성 (Generator)
```
[1] generate_workflow.py
    ├── fetch_latest_session_events() → 가장 최근 세션 가져오기
    ├── construct_prompt()            → LLM용 프롬프트 구성
    │   예: "User Actions Log:
    │         - [09:10] App: Excel | Action: Clicked Button '저장'
    │         Generate Python code to automate this."
    ├── generate_code()
    │   ├── OpenAI(api_key).chat.completions.create()
    │   └── model="gpt-4o-mini"
    └── 결과를 generated_workflow.py에 저장
```

---

## 7. 설정 파일 상세

### 7.1 `configs/config.yaml`
```yaml
# 수집기 서버
ingest:
  host: 127.0.0.1
  port: 8080

# 센서 설정
sensors:
  auto_start: true
  processes:
    - module: sensors.os.ui_automation
      enabled: true
      args: ["--interval", "0.5"]  # 폴링 간격 (초)
    - module: sensors.os.input_hook
      enabled: true
      args: ["--debounce-ms", "100"]  # 디바운스 간격 (밀리초)

# 데이터 보존
retention:
  raw_events_days: 7       # 원시 이벤트 보존 기간
  max_db_mb: 500           # DB 최대 크기
```

### 7.2 `configs/privacy_rules.yaml`
```yaml
# 허용된 앱 (이 앱들만 로그 수집)
allowlist_apps:
  - chrome.exe
  - excel.exe
  - outlook.exe
  - pycharm64.exe
  - slack.exe
  # 이 외에도 70개 이상의 앱이 허용됨 (아래 표 참조)
```

#### 허용된 앱 전체 목록

| 카테고리 | 앱 목록 |
| :--- | :--- |
| **시스템** | `applicationframehost.exe`, `systemsettings.exe`, `textinputhost.exe`, `windowsterminal.exe` |
| **웹 브라우저** | `chrome.exe`, `msedge.exe`, `edge.exe`, `whale.exe`, `firefox.exe`, `brave.exe`, `opera.exe`, `arc.exe` |
| **Microsoft Office** | `outlook.exe`, `excel.exe`, `winword.exe`, `powerpnt.exe`, `onenote.exe`, `onenotem.exe`, `msaccess.exe`, `visio.exe`, `project.exe`, `mspub.exe` |
| **클라우드 & 협업** | `onedrive.exe`, `sharepoint.exe`, `teams.exe`, `msteams.exe`, `ms-teams.exe`, `teamsbootstrapper.exe`, `lync.exe` |
| **화상회의** | `zoom.exe`, `webex.exe`, `skype.exe` |
| **메신저** | `slack.exe`, `discord.exe`, `kakaotalk.exe` |
| **노트 & 생산성** | `notion.exe`, `notion calendar.exe`, `microsoft.notes.exe`, `obsidian.exe`, `evernote.exe`, `trello.exe`, `jira.exe`, `confluence.exe` |
| **개발 도구** | `code.exe`, `devenv.exe`, `pycharm64.exe`, `githubdesktop.exe`, `gitkraken.exe`, `sourcetree.exe`, `postman.exe`, `insomnia.exe`, `docker desktop.exe`, `dockerdesktop.exe` |
| **디자인** | `figma.exe`, `figma agent.exe`, `adobe xd.exe`, `illustrator.exe`, `photoshop.exe`, `acrobat.exe` |
| **3D & 기타** | `7zfm.exe`, `3dsmax.exe`, `revit.exe`, `sketchup.exe`, `unity hub.exe`, `unity.exe` |

```yaml
# 차단된 앱 (절대 수집 안 함)

denylist_apps:
  - keepass
  - 1password
  - bitwarden

# PII 자동 마스킹
redaction_patterns:
  - name: email
    regex: '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
```

---

## 8. 데이터 모델

### 8.1 이벤트 구조 (`EventEnvelope`)
```python
@dataclass
class EventEnvelope:
    schema_version: str   # "1.0"
    event_id: str         # UUID
    ts: str               # ISO8601 타임스탬프
    source: str           # "ui_automation"
    app: str              # "Excel - Report.xlsx"
    event_type: str       # "user.interaction"
    priority: str         # "P0" (Critical), "P1" (High), "P2" (Normal)
    resource: ResourceRef # {"type": "ui_element", "id": "SaveButton"}
    payload: Dict         # 상세 정보 (아래 참조)
    privacy: PrivacyMetadata
    pid: int              # 프로세스 ID
    window_id: str        # 윈도우 핸들
```

### 8.2 Payload 예시 (`user.interaction`)
```json
{
  "window_title": "Report.xlsx - Excel",
  "control_type": "Button",
  "element_name": "저장",
  "element_value": "",
  "automation_id": "SaveButton",
  "bounding_rect": "(100, 200, 150, 230)"
}
```

---

## 9. 보안 및 프라이버시

### 9.1 데이터 수집 원칙
1.  **허용 목록 기반**: `allowlist_apps`에 명시된 앱만 수집.
2.  **차단 목록 우선**: `denylist_apps`에 있는 앱은 절대 수집 안 함 (비밀번호 관리자 등).
3.  **민감 정보 마스킹**: 이메일, 전화번호 등 PII는 자동으로 `[REDACTED]` 처리.

### 9.2 API 키 보호
- `.env` 파일에 저장 (Git에 커밋되지 않음).
- 환경 변수로 로드하여 코드에 노출되지 않음.

---

## 10. 확장 가능성 (향후 개발)

| 기능 | 설명 |
| :--- | :--- |
| **Execution Engine** | 생성된 코드를 검증 후 자동 실행 |
| **장기 패턴 분석** | 일주일/한 달 단위 반복 패턴 자동 감지 |
| **웹 UI 대시보드** | 수집된 데이터 시각화 및 관리 |
| **멀티 모니터 / Citrix 지원** | OCR 기반 fallback 센서 추가 |

---

## 11. 라이선스 및 크레딧

- **개발**: Antigravity with Human
- **주요 의존성**: Python, uiautomation, pynput, OpenAI API
- **라이선스**: (프로젝트 소유자 정의)

---

*이 문서는 2026-02-05 기준으로 작성되었습니다.*
