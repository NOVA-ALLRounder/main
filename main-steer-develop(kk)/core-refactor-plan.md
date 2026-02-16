# Core Refactor Plan

## Goal
`core` 모듈을 기능별로 분리해 유지보수성과 테스트 가능성을 높이고, 기존 동작은 유지한다.

## Tasks
- [x] `api_server.rs` 1차 분리: DCP/이메일워크플로우/화면품질/시스템유틸 모듈로 분리  
  Verify: `cargo check` 통과
- [x] `db.rs` 1차 분리: chat/routine/dashboard/policy 블록을 `db_aux.rs`로 이동  
  Verify: `cargo check` 통과
- [x] `db.rs` 2차 분리: recommendation 관련 조회/상태변경 로직 별도 모듈화 (`db_recommendation.rs`)  
  Verify: `cargo check` + 추천 API smoke
- [x] `db.rs` 3차 분리: NL run / verification run / quality score 로직 분리 (`db_analytics.rs`)  
  Verify: `cargo check` + 기존 메트릭 API 회귀 없음
- [x] `api_server.rs` 2차 분리: 라우트 등록부와 핸들러 구현 분리 (`api_server_routes.inc.rs`)  
  Verify: `cargo check` + 주요 엔드포인트 응답 확인
- [x] `api_server.rs` 3차 분리: 요청/응답 타입 분리 (`api_server_types.rs`)  
  Verify: `cargo check` 통과, 기존 핸들러 시그니처 유지
- [x] `api_server.rs` 4차 분리: 루틴/추천/품질/목표 핸들러 분리 (`api_server_handlers_ops.inc.rs`)  
  Verify: `cargo check` 통과, 라우트 시그니처 유지
- [x] `api_server.rs` 5차 분리: agent 핸들러 분리 (`api_server_handlers_agent.inc.rs`)  
  Verify: `cargo check` 통과, 라우트 시그니처 유지
- [x] `api_server.rs` 6차 분리: 세션 핸들러 분리 (`api_server_handlers_sessions.inc.rs`)  
  Verify: `cargo check` 통과, 라우트 시그니처 유지
- [x] `api_server` 구조 정리: include 핸들러를 단일 모듈(`api_server_handlers.rs`)로 집약  
  Verify: `cargo check` 통과, 기존 라우트/핸들러 참조 유지
- [x] `db.rs` 4차 분리: routine 관련 로직 분리 (`db_routines.rs`)  
  Verify: `cargo check` 통과
- [x] `db.rs` 5차 분리: exec approval/allowlist/result 로직 분리 (`db_exec.rs`)  
  Verify: `cargo check` 통과
- [ ] 중복/미사용 함수 정리 및 import 정리  
  Verify: `cargo check` 경고 감소, 기능 회귀 없음

## Done When
- [ ] `db.rs`와 `api_server.rs`가 각각 책임 단위 모듈로 분리되어 단일 파일 과대화가 완화됨
- [ ] 빌드(`cargo check`)가 지속적으로 통과함
- [ ] 통합 스모크(이메일/노션/텔레그램, 추천 API, 로그 API)가 깨지지 않음

- [x] api_server.rs 7th split: screen summary/chat helper block -> api_server_screen.inc.rs (cargo check pass)
- [x] api_server.rs 8th split: system/preflight/verification handlers -> api_server_system.inc.rs (cargo check pass)

- [x] api_server.rs 9th split: ingest_events block -> api_server_ingest.inc.rs (cargo check pass)
- [x] api_server.rs 10th split: handle_chat block -> api_server_chat.inc.rs (cargo check pass)

- [x] api_server.rs 11th split: handle_feedback/get_selection_context -> api_server_misc.inc.rs (cargo check pass)

- [x] api_server import cleanup: grouped/sorted crate and api_utils imports (cargo check pass)

- [x] api_server modularization: misc include -> api_server_misc.rs module (cargo check pass)
- [x] api_server modularization: system include -> api_server_system.rs module (cargo check pass)
- [x] api_server modularization: ingest include -> api_server_ingest.rs module (cargo check pass)
- [x] api_server modularization: chat include -> api_server_chat.rs module (cargo check pass)

- [x] api_server modularization: screen include -> api_server_screen.rs module (cargo check pass)
- [x] api_server include cleanup complete: major blocks now module wrappers over include files

- [x] api_server directory structuring: moved api_server related split files into core/src/api_server/ with role-based filenames (cargo check pass)

- [x] src root flattening (phase 1): moved api helper modules to core/src/api/ and wired via #[path] in lib.rs (cargo check pass)

- [x] src root flattening (phase 2): moved db helper modules to core/src/db/ and rewired db.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 3): moved verification modules to core/src/verification/ and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 4): moved agent workflow modules to core/src/agent/ and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 5): moved automation execution modules to core/src/automation/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 6): moved api_server_types/db_schema into domain dirs and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 7): moved schema/action_schema/workflow_schema into core/src/schemas/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 8): moved session/session_store to core/src/state/ and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 9): moved orchestrator/dynamic_controller/executor to core/src/runtime_core/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 10): moved replan modules to core/src/planning/ and config_manager to core/src/config/ with lib.rs #[path] updates (cargo check pass)

- [x] src root flattening (phase 11): moved security/policy/privacy/send_policy to core/src/security_core/ and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 12): moved monitor/notifier/scheduler/singleton_lock/dependency_check to core/src/ops/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 13): moved external_apis/mcp_client/llm_gateway/telegram to core/src/integration_core/ and rewired lib.rs #[path] entries (cargo check pass)
- [x] src root flattening (phase 14): moved applescript/browser_automation/screen_recorder/tool_chaining/visual_driver/permission_manager to core/src/ui_automation/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 15): moved analyzer/memory/content_extractor/reality_check (+ architect archival) to core/src/analysis_core/ and rewired lib.rs #[path] entries (cargo check pass)

- [x] src root flattening (phase 16): moved cli_llm/peekaboo_cli to core/src/cli_core/, paths/retry_logic to core/src/core_utils/, runtime_mode/main_startup to core/src/runtime_core/, subagent to core/src/agent/, collector_bridge to core/src/integration_core/ with lib.rs #[path] updates (cargo check pass)
