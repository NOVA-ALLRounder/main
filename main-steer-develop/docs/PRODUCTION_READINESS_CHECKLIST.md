# Production Readiness Checklist

Updated: 2026-02-10  
Scope: `main-steer-develop` (Tauri desktop + Rust core API + NL automation)

## 0) Release Gate (must pass)

- [ ] `core` API is always reachable on a single canonical port (`5680`).
- [ ] `.env` loading is deterministic (no BOM parse failure, single source of truth).
- [ ] Startup never fails due to stale/permission lock files.
- [ ] DB is writable in runtime path (no `readonly database` errors).
- [ ] LLM connectivity is stable in runtime (no repeated TLS/connect retries).
- [ ] Launcher never returns raw `Failed to reach agent.` for normal chat.

---

## 1) P0: Runtime Reliability (fix first)

### 1.1 Port consistency
- Files:
  - `start_steer.ps1`
  - `core/.env`
  - `web/src/lib/api.ts`
- Requirements:
  - Enforce API port `5680` across runtime and frontend default.
  - Remove conflicting fallback configs for `5681` in release workflow.
- Verification:
  - `netstat -ano | findstr :5680` must show `LISTENING`.
  - `Invoke-WebRequest http://127.0.0.1:5680/api/health` returns `ok`.

### 1.2 Env loading hardening
- Files:
  - `start_steer.ps1`
  - `core/src/main.rs` (log clarity)
- Requirements:
  - Copy `.env` as UTF-8 **without BOM** into runtime location.
  - Log exactly which `.env` path is loaded.
- Verification:
  - Runtime log must not contain `.env parse` errors.

### 1.3 Lock handling and single-instance safety
- Files:
  - `core/src/singleton_lock.rs`
  - `start_steer.ps1`
- Requirements:
  - If lock PID is dead, auto-clean lock file.
  - Handle access-denied lock paths with fallback to app-local lock dir.
- Verification:
  - Restart loop (`start/stop/start`) works 10 times without lock failure.

### 1.4 DB write reliability
- Files:
  - `core/src/db.rs`
  - `core/src/paths.rs`
- Requirements:
  - Ensure DB path points to writable directory for packaged runtime.
  - Fail fast with actionable error if not writable.
- Verification:
  - No `attempt to write a readonly database` in logs for 30 min soak run.

---

## 2) P0: Chat/Agent UX Stability

### 2.1 Normal chat must bypass fragile automation
- Files:
  - `web/src/features/launcher/useAgentWorkflow.ts`
- Requirements:
  - For short/low-confidence generic inputs, route directly to `/api/chat`.
  - Never show automation verify errors for greetings/small talk.
- Verification:
  - Inputs: `안녕`, `hi`, `고마워` always return chat response, not verify failure.

### 2.2 Error surface quality
- Files:
  - `web/src/lib/api.ts`
  - `web/src/features/launcher/Launcher.tsx`
- Requirements:
  - Replace generic `Failed to reach agent.` with classified errors:
    - API unreachable
    - auth/cors
    - timeout
    - LLM unavailable
- Verification:
  - Simulated failures produce expected user-readable message classes.

---

## 3) P1: LLM Connectivity and Policy

### 3.1 LLM network compatibility
- Files:
  - `core/src/llm_gateway.rs`
- Requirements:
  - Add robust retry classification (TLS vs timeout vs auth).
  - Avoid background flood retries for embeddings when network is unavailable.
- Verification:
  - Under blocked network, system degrades gracefully without runaway logs.

### 3.2 API key and model validation at startup
- Files:
  - `core/src/main.rs`
  - `scripts/test_openai_key.ps1`
- Requirements:
  - On startup, validate key/model once and cache result.
  - Expose status endpoint field: `llm_ready: true/false`.
- Verification:
  - `/api/system/health` includes explicit LLM readiness.

---

## 4) P1: Observability

### 4.1 Structured logs
- Files:
  - `core/src/main.rs`
  - `core/src/api_server.rs`
- Requirements:
  - Add request id/session id on API logs.
  - Normalize startup/runtime errors to machine-parseable format.
- Verification:
  - Can trace one user input across `intent -> plan -> execute -> verify`.

### 4.2 Minimal production metrics
- Requirements:
  - Track: request success rate, fallback rate, p95 latency, agent completion rate.
- Verification:
  - Metrics visible in one place (file/db/dashboard).

---

## 5) P1: Security and Guardrails

### 5.1 API auth and CORS policy cleanup
- Files:
  - `core/src/api_server.rs`
- Requirements:
  - Keep Tauri origins needed for runtime, remove unnecessary wildcards.
  - Validate Authorization behavior with and without `STEER_API_KEY`.
- Verification:
  - Security checks pass for expected origins only.

### 5.2 Critical action policy tests
- Files:
  - `core/src/policy.rs`
- Requirements:
  - Ensure critical shell/terminate paths require explicit env gates.
- Verification:
  - Unit tests cover allow/deny matrix.

---

## 6) P2: Delivery Pipeline

### 6.1 Build pipeline simplification
- Files:
  - `start_steer.ps1`
- Requirements:
  - Add `-RunOnly` mode (no rebuild) for rapid restart/debug.
  - Keep `-BuildAndRun` for clean release flow.
- Verification:
  - Dev restart < 5 seconds when binaries already built.

### 6.2 Preflight checks before launch
- Requirements:
  - Validate `.env`, API port availability, writable DB dir, lock status.
- Verification:
  - Launch script blocks with clear fix instructions when preflight fails.

---

## 7) Exit Criteria for “Production Ready”

- [ ] 7-day local soak with no blocking startup failures.
- [ ] 95%+ successful responses on normal chat inputs.
- [ ] 99% API health uptime during soak.
- [ ] No unresolved P0/P1 checklist items.
- [ ] Reproducible launch and rollback procedure documented.

