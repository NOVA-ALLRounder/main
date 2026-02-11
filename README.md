# 로컬 OS 자동화 워크플로우 (DCP + n8n)

이 프로젝트는 **Windows 환경에서 업무 활동 로그(UI Action)를 수집/분석하고, LLM 기반으로 반복 작업을 감지하여 n8n 워크플로우를 자동 생성/추천**하는 초개인화 자동화 파이프라인입니다.

모든 데이터는 로컬 DB에 저장되며, 민감 정보는 철저히 마스킹/암호화되어 관리됩니다.

---

## 🚀 핵심 목표
1. **정밀한 행동 수집**: 단순 앱 사용 시간이 아닌, **"어떤 버튼을 누르고 무엇을 입력했는지"** 구체적 행동(Action) 단위로 수집
2. **패턴 자동 감지**: 사용자가 반복적으로 수행하는 행동 시퀀스(예: 매일 아침 'daily report' 검색)를 자동 감지
3. **LLM 기반 자동화**: 감지된 패턴을 LLM에게 전달하여 n8n 워크플로우(JSON) 자동 생성
4. **개인화 루프**: 추천 → 사용자 피드백(승인/거절) → 학습을 통한 정확도 향상

---

## 🛠 아키텍처 & 데이터 흐름

```mermaid
flowchart TD
    subgraph Data Collection [1. 데이터 수집]
        Sensor[Windows Sensors] -->|UI Automation| Actions[Actions (Click/Input)]
        Sensor -->|Win32 API| Events[Events (App Focus)]
        Actions -->|N-gram 분석| Patterns[Pattern Analyzer]
        Patterns -->|반복 시퀀스 감지| LocalDB[(SQLite DB)]
        Events --> LocalDB
    end

    subgraph Data Processing [2. 데이터 가공]
        LocalDB -->|1일 1회| Summary[build_daily_summary.py]
        Summary -->|Action 요약 + 패턴 추출| DailyJson[Daily Summary JSON]
        DailyJson -->|LLM 최적화| LLMInput[build_llm_input.py]
    end

    subgraph AI & Automation [3. AI 자동화 생성]
        LLMInput -->|Prompt Engineering| LLM[LLM (OpenAI/Local)]
        LLM -->|Automation Idea| Generator[generate_n8n_workflow.py]
        Generator -->|n8n Workflow JSON| N8N[n8n Automation]
    end
```

### 1. 데이터 수집 (Collector)
- **Actions**: 클릭, 입력, 단축키, 스크롤 등 구체적 UI 조작 수집 (Electron 앱 포함)
- **Events**: 앱 포커스 전환, 창 제목 변경, 파일 생성 등 시스템 이벤트 수집
- **Pattern Analyzer**: 실시간으로 Action 스트림을 분석하여 반복 패턴 감지
    - 예: `Chrome 주소창 클릭` → `daily report 입력` → `Enter` (3회 반복 시 패턴으로 저장)
    - **Content-Aware**: "daily report"와 "weekly report"를 구별하되, 이메일/URL 등 민감 정보는 마스킹 처리

### 2. 데이터 가공 (Pipeline)
- **Daily Summary**: 하루 동안의 모든 Events와 Actions를 집계
    - 앱별 사용 시간, 주요 전환 경로(Transition)
    - **Action Steps**: 앱별 구체적 행동 빈도 (예: 저장 단축키 50회, 검색 버튼 클릭 20회)
    - **Repeated Sequences**: 가장 자주 반복된 행동 시퀀스 TOP 5
- **LLM Input Builder**: 가공된 요약 데이터를 LLM이 이해하기 쉬운 압축 포맷으로 변환

### 3. 워크플로우 생성 (Automation)
- **Recommendation Engine**: LLM이 사용자의 반복 패턴을 분석하여 "이 작업을 자동화할까요?" 제안
- **n8n Integration**: 사용자가 승인하면 즉시 n8n 워크플로우(JSON)로 변환하여 실행 가능한 상태로 배포

---

## 📂 프로젝트 구조

```
C:\os_final
├─ collector\Data-Collection-Projection
│  ├─ src\collector\
│  │  ├─ sensors\              # Windows UI Automation 센서 구현
│  │  ├─ pattern_analyzer.py   # 행동 패턴 감지 로직 (N-gram)
│  │  ├─ action_collector.py   # Action 데이터 수집 및 DB 저장
│  │  └─ logging_.py           # 로깅 포맷터 (Emoji 지원)
│  ├─ scripts\
│  │  ├─ build_daily_summary.py # 일일 요약 생성 (Action 통합)
│  │  ├─ build_llm_input.py     # LLM 입력 생성 (Action Steps 통합)
│  │  └─ ...
│  ├─ configs\
│  │  ├─ config.yaml           # 수집/보존 정책 설정
│  │  └─ privacy_rules.yaml    # 앱 차단/허용 및 마스킹 규칙
│  └─ logs\                    # 실행 로그 및 요약 파일 저장소
└─ ...
```

---

## 💻 실행 방법

### 1. 환경 설정
```powershell
# 가상환경 생성 및 패키지 설치
conda create -n DATA_C python=3.11 -y
conda activate DATA_C
pip install -r collector\Data-Collection-Projection\requirements.txt

# DB 초기화
python collector\Data-Collection-Projection\scripts\init_db.py --config collector\Data-Collection-Projection\configs\config.yaml
```

### 2. 수집기 실행 (Collector)
```powershell
# 모든 데이터 수집 시작
$env:PYTHONPATH="src"
python -m collector.main --config configs/config.yaml
```
- 실행하면 콘솔에 `[클릭]`, `[입력]` 등의 로그가 실시간으로 표시됩니다.
- 종료하려면 `Ctrl+C`를 누르세요.

### 3. 파이프라인 수동 실행 (데이터 가공 ~ 추천)
```powershell
# 1. 일일 요약 생성 (오늘 날짜)
python scripts\build_daily_summary.py --date 2026-02-11

# 2. LLM 입력 생성
python scripts\build_llm_input.py --daily logs\daily_summary_2026-02-11.json

# 3. 워크플로우 추천 생성
python scripts\generate_workflow_recommendations.py --input logs\llm_input.json
```

---

## 🔒 개인정보 보호 (Privacy)

사용자의 모든 활동을 수집하므로 개인정보 보호가 최우선입니다.

1. **로컬 저장 원칙**: 모든 raw data는 사용자 PC(`collector.db`)에만 저장되며 외부로 전송되지 않습니다.
2. **마스킹 처리**:
    - 이메일, 주민번호, 전화번호, 카드번호 등은 수집 즉시 `{EMAIL}`, `{NUM}` 등으로 마스킹됩니다.
    - `privacy_rules.yaml`에서 마스킹 규칙을 커스텀할 수 있습니다.
3. **앱 차단 (Denylist)**:
    - 비밀번호 관리자(1Password, LastPass 등), 금융 앱, 사생활 관련 앱은 수집에서 아예 제외됩니다.
4. **LLM 전송 최소화**:
    - LLM에게는 원본 데이터가 아닌, **철저히 요약되고 익명화된 통계 데이터**만 전송됩니다.

---

## 🔧 주요 설정 (config.yaml)

| 설정 항목 | 설명 | 기본값 |
|-----------|------|--------|
| `retention.raw_events_days` | 원본 이벤트 보존 기간 | 7일 |
| `retention.max_db_mb` | DB 최대 용량 제한 | 500MB |
| `logging.activity_to_console` | 콘솔에 실시간 활동 출력 여부 | true |
| `privacy.allowlist_apps` | 수집 허용 앱 목록 (비어있으면 전체 허용) | [] |

---

## 🐞 트러블슈팅

Q: **콘솔에 `unknown`이라고만 떠요.**
A: Electron 기반 앱(VS Code, Discord 등)이나 보안이 강화된 앱은 내부 UI 구조를 OS에 노출하지 않아 `control_name`을 가져오지 못할 수 있습니다. 하지만 **창 제목(Window Title)**과 **입력 내용**은 정상 수집되므로 패턴 분석에는 지장이 없습니다.

Q: **DB 용량이 너무 커지지 않나요?**
A: `config.yaml`의 `max_db_mb` 설정에 따라 오래된 데이터부터 자동으로 삭제(Vacuum)됩니다. 기본값은 500MB이며, 텍스트 데이터 위주라 효율적으로 압축됩니다.
