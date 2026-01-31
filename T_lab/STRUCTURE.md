# 📁 T_lab 프로젝트 구조 상세

> 이 문서는 T_lab의 디렉토리 및 파일 구조를 상세히 설명합니다.

---

## 루트 구조

```
T_lab/
├── README.md              # 종합 문서 (아키텍처, API, 설치 방법)
├── STRUCTURE.md           # 이 파일 (구조 상세)
├── docker-compose.yml     # Docker 멀티 컨테이너 설정
├── t_lab.db              # SQLite 데이터베이스 (루트 레벨)
├── .env                  # 환경 변수 (복사본)
├── .env.example          # 환경 변수 템플릿
│
├── apps/                 # 애플리케이션 모노레포
│   ├── api/             # Backend (FastAPI)
│   └── web/             # Frontend (Next.js)
│
└── scripts/             # 유틸리티 스크립트
    └── init_db.py       # DB 초기화
```

---

## Backend (`apps/api/`)

### 핵심 파일

| 파일 | 설명 | 주요 내용 |
|------|------|----------|
| `main.py` | FastAPI 메인 앱 | API 엔드포인트, PersistentStore, WebSocket |
| `workflow.py` | LangGraph 워크플로우 | 노드 정의, 그래프 빌더 |
| `state.py` | ScientificState 정의 | TypedDict 상태 스키마 |
| `models.py` | SQLAlchemy 모델 | Session 테이블 정의 |
| `connection_manager.py` | WebSocket 관리자 | 연결 관리, 브로드캐스트 |

### 상세 구조

```
apps/api/
│
├── main.py                     # [501 lines] FastAPI 메인 앱
│   ├── PersistentStore         # 세션 CRUD
│   ├── ResearchRequest         # API 모델
│   ├── start_research()        # POST /research/start
│   ├── select_method()         # POST /research/select-method
│   ├── websocket_endpoint()    # WebSocket 핸들러
│   └── generate_pdf()          # PDF 생성
│
├── workflow.py                 # [264 lines] LangGraph 정의
│   ├── router_node()           # 의도 분류
│   ├── librarian_node()        # 문헌 검색
│   ├── pi_node_novelty()       # 독창성 평가
│   ├── pi_node_methods()       # 방법론 제안 (H₀/H₁ 로깅)
│   ├── engineer_node()         # 코드 생성 (방법론 로깅)
│   ├── runner_node()           # 시뮬레이션 (파라미터 로깅)
│   ├── author_node()           # 보고서 작성
│   ├── critic_node()           # 비판적 검토
│   ├── fact_checker_node()     # 인용 검증
│   ├── build_research_graph()  # 초기 분석 그래프
│   └── build_execution_graph() # 실험 실행 그래프
│
├── state.py                    # ScientificState TypedDict
│
├── models.py                   # SQLAlchemy Session 모델
│
├── connection_manager.py       # WebSocket ConnectionManager
│
├── requirements.txt            # Python 의존성
│
├── agents/                     # 에이전트 모듈 (10개)
│   ├── __init__.py
│   ├── router.py              # [180 lines] 의도 분류
│   ├── librarian.py           # [189 lines] 문헌 검색
│   ├── pi.py                  # [270 lines] 연구 설계
│   ├── engineer.py            # [310 lines] 코드 생성
│   ├── experiment_runner.py   # [280 lines] 시뮬레이션
│   ├── author.py              # [230 lines] 보고서 작성
│   ├── critic.py              # [150 lines] 비판적 검토
│   ├── fact_checker.py        # [100 lines] 인용 검증
│   ├── paper_synthesizer.py   # [150 lines] 논문 합성
│   └── preregistrar.py        # [140 lines] (비활성화)
│
├── core/                       # 핵심 유틸리티
│   ├── __init__.py
│   ├── config.py              # Settings (pydantic)
│   ├── database.py            # DB 연결 (SQLAlchemy)
│   └── logging.py             # structlog 로거
│
├── tools/                      # 도구 모듈
│   └── pdf_generator.py       # ReportLab PDF 생성
│
├── static/                     # 생성된 static 파일
│   └── {session_id}_result.png # 실험 결과 이미지
│
├── reports/                    # 저장된 Markdown 보고서
│   └── report_{session_id}_{timestamp}.md
│
├── tmp_pdfs/                   # PDF 임시 파일
│   └── research_{session_id}.pdf
│
└── t_lab.db                    # SQLite 데이터베이스
```

---

## 에이전트 상세 (`apps/api/agents/`)

### 처리 흐름 순서

```
1. router.py         → 의도 분류 (hypothesis / question)
2. librarian.py      → (question일 때) 문헌 검색 및 응답
3. pi.py             → (hypothesis일 때) 독창성 평가 + 방법론 제안
4. engineer.py       → 선택된 방법론 기반 코드 생성
5. experiment_runner.py → 몬테카를로 시뮬레이션 실행
6. author.py         → IMRAD 보고서 작성
7. critic.py         → 보고서 비판적 검토
8. fact_checker.py   → 인용 검증
9. paper_synthesizer.py → (별도) 다중 세션 논문 합성
```

### 에이전트별 핵심 클래스/함수

| 파일 | 클래스 | 진입점 함수 |
|------|--------|------------|
| router.py | RouterAgent | `classify_intent(state)` |
| librarian.py | LibrarianAgent | `search_literature(state)` |
| pi.py | PIAgent | `evaluate_novelty(state)`, `propose_methods(state)` |
| engineer.py | EngineerAgent | `execute_experiment(state)` |
| experiment_runner.py | ExperimentRunnerAgent | `run_simulation(state)` |
| author.py | AuthorAgent | `write_report(state)` |
| critic.py | CriticAgent | `review_report(state)` |
| fact_checker.py | FactCheckerAgent | `verify_citations(state)` |
| paper_synthesizer.py | PaperSynthesizer | `synthesize_papers(sessions)` |

---

## Frontend (`apps/web/`)

### 주요 구조

```
apps/web/
│
├── src/
│   ├── app/                      # Next.js App Router
│   │   ├── page.tsx             # [600+ lines] 메인 페이지
│   │   │   ├── Input Screen      # 가설 입력
│   │   │   ├── Method Selection  # 방법론 선택
│   │   │   ├── Running Screen    # 실험 진행 (실시간)
│   │   │   │   ├── 실험 설계 카드
│   │   │   │   ├── Activity Console
│   │   │   │   └── 실시간 차트
│   │   │   └── Complete Screen   # 결과 표시
│   │   │
│   │   ├── page.module.css      # 메인 스타일
│   │   ├── globals.css          # 글로벌 CSS
│   │   ├── layout.tsx           # 앱 레이아웃
│   │   │
│   │   ├── experiments/         # 실험 상세 페이지
│   │   │   └── [id]/
│   │   │       └── page.tsx
│   │   │
│   │   └── papers/              # 논문 합성 페이지
│   │       └── page.tsx
│   │
│   ├── components/              # React 컴포넌트
│   │   ├── SimulationChart.tsx  # Recharts 시뮬레이션 차트
│   │   ├── SimulationChart.module.css
│   │   ├── ExperimentReport.tsx # 보고서 렌더링
│   │   └── ExperimentReport.module.css
│   │
│   └── lib/
│       └── api/
│           └── client.ts        # API 클라이언트 + 타입 정의
│               ├── Session       # 세션 인터페이스
│               ├── Method        # 방법론 인터페이스 (H₀/H₁)
│               ├── SimulationParams  # 시뮬레이션 파라미터
│               └── api           # API 함수들
│
├── public/                      # 정적 자산
├── next.config.mjs              # Next.js 설정
├── package.json                 # npm 패키지
└── tsconfig.json                # TypeScript 설정
```

### 주요 UI 컴포넌트

| 컴포넌트 | 위치 | 기능 |
|----------|------|------|
| Input Screen | page.tsx | 가설/질문 입력, 도메인 설정 |
| Method Selection | page.tsx | 3가지 방법론 카드 표시, 선택 |
| Experiment Design Card | page.tsx | 선택된 방법론, H₀/H₁, 파라미터 표시 |
| Activity Console | page.tsx | 실시간 에이전트 활동 로그 |
| Simulation Charts | page.tsx | P-value, Power 실시간 차트 |
| Literature Warning | page.tsx | 문헌 반박 경고 배너 |
| Complete Screen | page.tsx | 결과, 보고서, PDF 다운로드 |

---

## 데이터 흐름

### 세션 상태 전이

```
새 세션 생성
    ↓
[running] → Router → Librarian/PI
    ↓
[paused] → 방법론 선택 대기
    ↓
(사용자 선택)
    ↓
[running] → Engineer → Runner → Author → Critic → FactChecker
    ↓
[completed] → 결과 표시, PDF 생성 가능
```

### WebSocket 메시지 흐름

```
Frontend                         Backend
   │                                │
   ├── ws connect ─────────────────→│
   │                                │
   │←── log (Router 시작) ──────────┤
   │←── log (PI 방법론 설계) ───────┤
   │←── log (H₀/H₁ 가설) ───────────┤
   │                                │
   │←── data_point (iter=10) ───────┤
   │←── data_point (iter=20) ───────┤
   │←── ...                         │
   │←── data_point (iter=100) ──────┤
   │                                │
   │←── log (시뮬레이션 완료) ───────┤
   │←── log (보고서 작성) ───────────┤
   │                                │
   ├── ws close ───────────────────→│
```

---

## 환경 변수

### `.env` 파일

```ini
# Required
OPENAI_API_KEY=sk-...

# Optional
DEFAULT_MODEL=gpt-4o
DATABASE_URL=sqlite:///t_lab.db
MAX_LITERATURE_RESULTS=10
MAX_SIMULATION_ITERATIONS=100
LOG_LEVEL=INFO
```

---

## 주요 의존성

### Backend (`requirements.txt`)

```
fastapi>=0.109.0
uvicorn>=0.27.0
langchain>=0.1.0
langchain-openai>=0.0.5
langgraph>=0.0.20
sqlalchemy>=2.0.0
pydantic>=2.5.0
httpx>=0.26.0
numpy>=1.24.0
scipy>=1.11.0
matplotlib>=3.8.0
reportlab>=4.0.0
structlog>=24.1.0
python-dotenv>=1.0.0
```

### Frontend (`package.json`)

```json
{
  "dependencies": {
    "next": "14.0.0",
    "react": "^18.2.0",
    "recharts": "^2.10.0",
    "react-markdown": "^9.0.0",
    "remark-gfm": "^4.0.0"
  }
}
```

---

## 빠른 참조

### 새 에이전트 추가하기

1. `apps/api/agents/`에 새 파일 생성
2. `{Agent}Agent` 클래스 및 진입점 함수 정의
3. `workflow.py`에 노드 함수 추가
4. `build_*_graph()`에 노드와 엣지 추가

### 새 API 엔드포인트 추가하기

1. `apps/api/main.py`에 라우트 함수 추가
2. Request/Response 모델 정의
3. Frontend `lib/api/client.ts`에 API 함수 추가

### 새 UI 컴포넌트 추가하기

1. `apps/web/src/components/`에 컴포넌트 생성
2. `apps/web/src/app/page.tsx`에 import 및 사용
3. `page.module.css`에 스타일 추가

---

> 📁 **T_lab Structure Document** - Last updated: 2026-01-28
