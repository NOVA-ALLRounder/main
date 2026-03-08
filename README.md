# Local OS Agent (Rust Native)

사용자 행동을 분석해 자동화를 추천/실행하는 macOS 기반 에이전트입니다.

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust)](https://www.rust-lang.org/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple)](https://www.apple.com/macos/)

## 한눈에 보기

- 목적: 사용자 행동 기반 자동화 추천 + 실행
- 런타임: Rust core + (선택) n8n + Tauri UI
- 권장 OS: macOS 12+
- 핵심 문서: `docs/CONFIG.md`, `docs/SPEC.md`, `docs/BUILD_DEPLOY_RUNBOOK.md`

## 5분 시작

```bash
# 1) clone
git clone <repo_url>
cd local-os-agent

# 2) 환경변수
cp core/.env.example core/.env
# core/.env 에 API 키 등 필수 값 입력

# 3) 빌드
cargo build --manifest-path core/Cargo.toml --release

# 4) 실행 (개발)
STEER_RUNTIME_PROFILE=dev STEER_API_ALLOW_NO_KEY=1 ./core/target/release/local_os_agent

# 5) 실행 (운영 권장)
STEER_RUNTIME_PROFILE=prod STEER_API_KEY=your_key ./core/target/release/local_os_agent
```

## 핵심 기능/명령

| 기능 | 명령어 | 설명 |
|:---|:---|:---|
| Routine 분석 | `routine` | 일일 루틴 분석 |
| 자동화 추천 | `recommend` | 자동화 스크립트 제안 |
| 앱 제어 | `control <app> <cmd>` | 앱 내부 제어 |
| 워크플로우 생성 | `build_workflow <prompt>` | n8n 워크플로우 생성 |
| 뉴스 Digest | `ai_digest [msg]` | 토픽 기반 digest 트리거 |
| 셸 실행 | `exec <cmd>` | 안전 정책 하 셸 명령 실행 |
| 상태 확인 | `status` | 시스템/런타임 상태 확인 |

## 실행 모드

### Core 실행

```bash
# 개발 로컬 모드
STEER_RUNTIME_PROFILE=dev STEER_API_ALLOW_NO_KEY=1 ./core/target/release/local_os_agent

# UI 데모 모드(백그라운드 간섭 최소화)
STEER_RUNTIME_PROFILE=dev STEER_API_ALLOW_NO_KEY=1 STEER_DISABLE_EVENT_TAP=1 ./core/target/release/local_os_agent

# 복구 + 백그라운드 재기동
./scripts/recover_runtime.sh
```

### n8n 모드 (macOS 권장)

기본값은 `manual`입니다.

- 기본: `STEER_N8N_RUNTIME=manual`
- 대체: `STEER_N8N_RUNTIME=docker` 또는 `STEER_N8N_RUNTIME=npx`
- docker 빠른 시작:

```bash
docker compose up -d n8n
# 기본 API URL: http://localhost:5678/api/v1
```

세부 정책/옵션은 [docs/CONFIG.md](docs/CONFIG.md) 참고.

## 릴리즈/배포

```bash
./scripts/rebuild_and_deploy.sh
```

이 스크립트가 자동 수행하는 작업:

1. Core 빌드 (`core/target/release/local_os_agent`)
2. Sidecar 동기화 (`web/src-tauri/binaries/core-aarch64-apple-darwin`)
3. Tauri 번들 빌드 (`web/`에서 `npm run tauri build`)
4. 앱 교체 + API 헬스 확인 (`/Applications/Steer OS.app`)

자세한 절차: [docs/BUILD_DEPLOY_RUNBOOK.md](docs/BUILD_DEPLOY_RUNBOOK.md)

빠른 개발 루프:

```bash
./scripts/validate_core_cli.sh --goal "메모장 열어서 박대엽이라고 써줘"
# 최종 배포 시 1회
./scripts/rebuild_and_deploy.sh
```

## 테스트/시연

### 테스트

```bash
cargo test --manifest-path core/Cargo.toml
```

출시용 자연어/메모리/추천 게이트 평가:

```bash
cargo run --manifest-path core/Cargo.toml --bin launch_eval
# 또는 다른 시나리오 파일 사용
cargo run --manifest-path core/Cargo.toml --bin launch_eval -- configs/launch_eval.yaml
# 실사용 로그 기반 후보를 generated YAML로 갱신한 뒤 바로 평가
cargo run --manifest-path core/Cargo.toml --bin launch_eval -- --refresh-candidates --candidate-limit 20
# 내부 dogfood용 synthetic 후보 snapshot 생성
cargo run --manifest-path core/Cargo.toml --bin launch_eval -- --refresh-candidates --candidate-mode synthetic --candidate-limit 20
# 출시용 readiness 리포트 생성
cargo run --manifest-path core/Cargo.toml --bin release_readiness -- --save-baseline
# 실제 HTTP 서버 live smoke
cargo run --manifest-path core/Cargo.toml --bin http_e2e
# daily launch readiness run (먼저 live HTTP E2E 실행, 자동 archive 포함, baseline 보존)
bash scripts/release_readiness_daily.sh
# baseline을 명시적으로 다시 저장해야 할 때만
bash scripts/release_readiness_daily.sh --save-baseline
```

- 기본 시나리오 파일: `configs/launch_eval.yaml`
- generated 후보 파일: `configs/launch_eval.generated.yaml`
- dogfood snapshot 파일: `configs/launch_eval.dogfood.generated.yaml`
- 결과 산출물: `reports/launch_eval/latest.json`, `reports/launch_eval/latest.md`
- 통합 출시 점검 리포트: `reports/release_readiness/latest.json`, `reports/release_readiness/latest.md`
- live HTTP E2E 리포트: `reports/http_e2e/latest.json`, `reports/http_e2e/latest.md`
- live HTTP E2E history: `reports/http_e2e/history/<timestamp>/`
- daily readiness history: `reports/release_readiness/history/<timestamp>/`
- each `release_readiness` run now writes its own archive paths into `reports/release_readiness/latest.json`
- Dashboard의 `Run release readiness`는 read-only다. baseline 저장은 `Set release baseline`이나 `--save-baseline` 경로에서만 수행한다.
- launch/release/http E2E 운영 API는 기본적으로 서버 현재 workdir만 사용한다. 클라이언트가 보내는 `workdir` override는 내부 smoke/test env를 제외하면 무시된다.
- `Set release baseline`도 서버 고정 baseline request로만 저장된다. 클라이언트가 `workdir/config/path/limit`를 덮어쓰는 건 기본적으로 차단된다.
- 출시 운영 런북: `docs/RELEASE_READINESS_RUNBOOK.md`
- 검증 범위:
  - deterministic chat intent
  - request/execution memory 재사용
  - memory scope isolation
  - work-only recommendation queue / approval gate
  - 실제 Axum 서버 기반 HTTP source-of-truth smoke (`http_e2e`)
  - `http_e2e`가 `latest/history` 운영 API와 `release_readiness latest/history` API까지 live HTTP로 검증
  - `release_readiness`가 최신 `http_e2e` 리포트를 읽고 blocker/advisory에 반영
  - Dashboard에서 최근 `HTTP E2E` history 확인 가능
  - release readiness trend summary에 `HTTP E2E` pass-rate drift가 같이 표시됨
  - Settings의 `Launch Eval Candidates`에서 `Real`과 `Dogfood` 후보를 분리해서 snapshot 저장 가능
  - Settings에서 memory admin audit trail과 recommendation review audit trail 확인 가능
  - Launch Readiness Ops에서 recommendation review friction metrics 확인 가능
  - release gate current payload에도 recommendation review friction이 포함됨

GUI 회귀 테스트:

```bash
bash run_gui_regression_pack.sh
STEER_GUI_REG_PACK_REPEAT=3 bash run_gui_regression_pack.sh
STEER_GUI_REG_SCENARIOS=1,3,5 bash run_gui_regression_pack.sh
```

### 시연 자동화

```bash
# 원클릭 시연
./scripts/demo_run.sh --preset news_telegram

# 사용자 프롬프트 시연
./scripts/demo_run.sh --prompt "오늘 받은 메일 5개 요약해줘"
```

관련 스크립트: `scripts/demo_prep.sh`, `scripts/demo_state_reset.sh`, `scripts/record_demo_preset.sh`

## Rust Collector (권장 경로)

Python collector 대신 Rust 단일 바이너리로 수집/집약을 실행할 수 있습니다.

```bash
cargo build --manifest-path core/Cargo.toml --bin collector_rs
STEER_DB_PATH=./steer.db STEER_COLLECTOR_PORT=8080 ./core/target/debug/collector_rs
```

- 엔드포인트: `POST /events`, `GET /health`, `GET /stats`
- 기본 로컬 실행 스크립트: `bash scripts/run_local.sh`
- 데이터 파이프라인 문서: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## 보안/운영 체크

- `exec` 명령은 위험 키워드가 포함되면 차단됩니다.
- 기본적으로 Write Lock이 활성화됩니다.
- 릴리즈 전 워크트리 안전 체크 권장:

```bash
./scripts/check_release_worktree.sh
./scripts/check_release_worktree.sh --staged
```

운영 정책/환경변수/진단 이벤트/승인 정책 전체 목록은 [docs/CONFIG.md](docs/CONFIG.md) 참고.

## 문서 맵

### 핵심 문서

- [아키텍처](docs/ARCHITECTURE.md)
- [설정/환경변수](docs/CONFIG.md)
- [보안](docs/SECURITY.md)
- [전체 스펙](docs/SPEC.md)
- [툴 인터페이스](docs/TOOL_INTERFACE.md)
- [롤아웃 체크리스트](docs/ROLLOUT_CHECKLIST.md)

### 자동화 로드맵/체크

- [NL 자동화 Phase0 스펙](docs/NL_AUTOMATION_PHASE0_SPEC.md)
- [NL 자동화 로드맵](docs/NL_AUTOMATION_ROADMAP.md)
- [NL 자동화 테스트 체크리스트](docs/NL_AUTOMATION_TEST_CHECKLIST.md)

### 추가 명세서 (docs/docs)

- [01_설계문서](docs/docs/명세서/01_설계문서.md)
- [02_서비스_설계_명세서](docs/docs/명세서/02_서비스_설계_명세서.md)
- [03_기능_설계_명세서](docs/docs/명세서/03_기능_설계_명세서.md)
- [04_개발_명세서](docs/docs/명세서/04_개발_명세서.md)
- [05_DB_설계_명세서](docs/docs/명세서/05_DB_설계_명세서.md)

## 프로젝트 구조

```text
core/                  # Rust core (agent/runtime/collector)
web/                   # Tauri UI
scripts/               # 실행/복구/시연/검증 스크립트
configs/               # runtime/pipeline 설정
docs/                  # 운영/설계/명세 문서
workflows/             # n8n/생성 워크플로우 산출물
```

## 라이선스

MIT License
