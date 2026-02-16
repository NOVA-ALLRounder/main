# Steer (main-steer-develop)

This README is focused on features that are currently implemented and runnable in this repository.

## What Is Working Now (Verified)

### Core runtime and dev stack
- `scripts/dev-up.ps1`: starts core API + web dev server
- `scripts/dev-status.ps1`: checks core/web health
- `scripts/dev-down.ps1`: stops tracked processes

### Core API smoke checks (Windows)
Verified by `scripts/smoke_core_windows.ps1`:
- `GET /api/health`
- `GET /api/context/selection`
- `GET /api/system/health`

### API server endpoints (implemented)
Main groups available in `core/src/api_server/routes.inc.rs`:
- system: health, preflight, mode, emergency-stop
- events/chat: ingest events, chat
- recommendations/routines
- exec approvals/allowlist/results
- verification: runtime/visual/semantic/performance/consistency + runs
- quality scoring + latest
- agent workflow endpoints (intent/plan/execute/verify/approve)
- sessions + resume

### Data and performance path
- SQLite-backed core DB (`core/src/db.rs`)
- event ingestion path supports batch insert optimization (`insert_events_v2_batch`)

## Conditional Features (Require Env/External Services)

These are implemented, but only work when credentials/services are configured:
- Gmail integration
- Notion integration
- Telegram integration
- n8n workflow creation/execution
- OpenAI LLM-backed responses

Use `core/.env` to configure required keys and IDs.

## Quick Start (Windows)

1. Create env file:
- copy `core/.env.example` to `core/.env`
- set at least `OPENAI_API_KEY` if you want LLM features

2. Start dev stack:
```powershell
./scripts/dev-up.ps1
```

3. Run smoke test:
```powershell
./scripts/smoke_core_windows.ps1
```

4. Stop dev stack:
```powershell
./scripts/dev-down.ps1
```

Default URLs:
- Core API: `http://127.0.0.1:5680`
- Web UI: `http://127.0.0.1:5173`

## Tauri App Run Modes (Windows)

From project root:

```powershell
cd C:\Users\Admin\Desktop\steer\main-steer-develop
```

Run packaged-style Tauri app (recommended):

```powershell
powershell -ExecutionPolicy Bypass -File .\start_steer.ps1 -SkipDcp
```

Run Tauri dev mode:

```powershell
powershell -ExecutionPolicy Bypass -File .\start_steer.ps1 -Dev -SkipDcp
```

Run core + web only (no Tauri window):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev-up.ps1
```

Stop core + web stack:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\dev-down.ps1
```

Stop Tauri window process explicitly:

```powershell
taskkill /IM app.exe /F
```

## Current Source Layout (Refactored)

`core/src` root is intentionally minimal:
- `api_server.rs`
- `db.rs`
- `error.rs`
- `lib.rs`
- `main.rs`

Domain folders include:
- `api_server/`, `api/`, `db/`, `verification/`, `agent/`, `automation/`
- `integration_core/`, `ui_automation/`, `analysis_core/`, `intelligence/`
- `ops/`, `runtime_core/`, `state/`, `security_core/`, `schemas/`, `config/`, `planning/`, `cli_core/`, `core_utils/`

## Build

- Windows:
```powershell
./scripts/build_release.ps1
```

- macOS/Linux:
```bash
./scripts/build_release.sh
```

## Notes

- If API does not come up, inspect `.runtime/logs`.
- If integration flows fail, check `core/.env` values first.
