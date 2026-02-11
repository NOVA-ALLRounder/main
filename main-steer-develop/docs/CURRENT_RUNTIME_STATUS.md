# Current Runtime Status and Capabilities

Updated: 2026-02-11
Scope: `main-steer-develop` (Tauri desktop + Rust core + Web UI)

## 1. What the app is right now

This project is a local desktop agent platform, not a fully autonomous "Jarvis" yet.

It currently provides:
- A Tauri desktop app (`web/src-tauri`) that runs a local Rust sidecar (`core.exe`).
- A local HTTP API (default `http://127.0.0.1:5680/api`).
- Chat-driven task routing for practical local actions.
- Health/version endpoints for runtime visibility.
- UI fallback path: if WebView HTTP fails, Tauri command proxy is used.

It does **not** currently provide:
- Fully reliable multi-step autonomous planning for all arbitrary tasks.
- Production-grade policy governance for all command classes.
- End-to-end regression test coverage for all action routes.

## 2. Runtime architecture (actual flow)

1. User types in desktop UI (React).
2. UI calls `sendChatMessage()` (`web/src/lib/api.ts`).
3. Primary path: local HTTP `POST /api/chat`.
4. Fallback path: Tauri invoke `proxy_chat` command (`web/src-tauri/src/lib.rs`), which calls local API directly from Rust.
5. Rust core (`core/src/api_server.rs`) routes request:
   - Fast-path commands (status/calendar/mail/greeting/windows actions/guarded shell)
   - Otherwise intent + LLM handling
6. Response is returned to UI and rendered as chat output.

## 3. Current practical capabilities

### 3.1 Chat and system information
- `status`, `system status` -> live CPU/RAM from local machine.
- Greeting handling (`hello`, `hi`) -> deterministic response.

### 3.2 Windows local actions (chat fast-path)
Implemented in `core/src/api_server.rs`:
- Open apps:
  - Notepad (`notepad`)
  - Calculator (`calculator`, `calc`)
  - Task Manager (`task manager`, `taskmgr`)
  - Windows Settings (`windows settings`)
  - Explorer (`explorer`)
  - Chrome (`open chrome`)
  - Edge (`open edge`)
- List files:
  - Current folder / Downloads / Desktop based on user text.
- Open URL:
  - `url open https://...`
- Web search:
  - `web search <query>`
- Email workflow bootstrap:
  - `email workflow`
  - `email workflow template run` (credentials 없이도 샘플 데이터로 전체 체인 테스트)
  - Produces `artifacts/email_ops/<timestamp>/summary.md` and `tasks.csv` when credentials are available.
  - Returns `error_missing_credentials` if Gmail credentials are not configured.

### 3.3 Guarded shell execution
Implemented in `core/src/api_server.rs` + `core/src/bash_executor.rs`:
- Prefixes:
  - `run <command>`
  - `cmd <command>`
  - localized alias is also supported in current build
- Executes in PowerShell on Windows.
- Returns exit code, duration, stdout/stderr summary.
- Basic destructive-pattern blocking exists (format/delete/shutdown registry patterns).

### 3.4 Integrations
- Calendar/Gmail command routes exist.
- If credentials are missing, API returns explicit configuration error.

## 4. Health and observability (current)

### 4.1 Health endpoint
`GET /api/system/health`

Current payload includes:
- `missing_deps`
- `api_port`
- `api_reachable`
- `llm_enabled`
- `operation_mode`
- `emergency_stop`
- `allow_automation`
- `analyzer_disabled`
- `background_analysis_disabled`
- `privacy_salt_set`
- `os`
- `checked_at_utc`

### 4.2 Runtime mode control endpoints
- `GET /api/system/mode`
- `POST /api/system/mode` with body `{ "mode": "observe|copilot|autopilot" }`
- `POST /api/system/emergency-stop` with body `{ "enabled": true|false }`

These gates now control action execution on Windows local action routes.
Blocked automation returns `error_action_denied`.

### 4.3 Version endpoint
`GET /api/version`

Current payload:
- `core_version`
- `build_profile` (`debug` or `release`)
- `git_sha` (if injected via env)

### 4.4 Logs
Runtime logs are written near Tauri release target:
- `core_sidecar.out.log`
- `core_sidecar.err.log`

## 5. Startup and operation

Primary startup script:
- `start_steer.ps1 -SkipDcp`

Current startup defaults include:
- `STEER_API_PORT=5680`
- `STEER_DISABLE_ANALYZER=1`
- `STEER_DISABLE_BACKGROUND_ANALYSIS=1`

Reason: reduce background LLM traffic and avoid token-rate exhaustion impacting chat UX.

## 6. Known limitations

- Some text output still has encoding artifacts in certain terminal/UI paths.
- Action routing is rule-based for many practical commands; open-domain reasoning still depends on LLM quality.
- Command blocking is currently pattern-based, not full policy engine coverage.
- Build is functioning, but large frontend chunk warnings remain (`vite` chunk size warning).

## 7. Recommended next production hardening steps

1. Add structured audit logs for all local action routes:
   - command, args, user text, result, duration, denied reason.
2. Expand command safety policy:
   - allowlist-by-default for high-risk environments.
3. Add automated tests for chat fast-path routes:
   - windows action commands, list-files, guarded shell block cases.
4. Add release metadata injection:
   - set `STEER_GIT_SHA` during build.
5. Add standard error codes in API responses:
   - `RATE_LIMIT`, `MISSING_CREDENTIALS`, `ACTION_DENIED`, `EXEC_FAILED`.

## 8. Source file map (important)

- Core API/router:
  - `core/src/api_server.rs`
- Health/dependency checks:
  - `core/src/dependency_check.rs`
- Command execution:
  - `core/src/bash_executor.rs`
- Windows native actions:
  - `core/src/windows/actions.rs`
- Frontend API client:
  - `web/src/lib/api.ts`
- Tauri fallback proxy:
  - `web/src-tauri/src/lib.rs`
- Startup orchestration:
  - `start_steer.ps1`
