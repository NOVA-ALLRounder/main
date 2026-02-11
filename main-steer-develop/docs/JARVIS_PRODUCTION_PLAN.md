# JARVIS Production Plan

Updated: 2026-02-11
Scope: `main-steer-develop`

## 1) Current Working Baseline

- Tauri-first app build and launch works via `start_steer.ps1 -SkipDcp`.
- Core API is reachable on `http://127.0.0.1:5680`.
- Chat core path is restored and compiling.
- Runtime mode controls are active:
  - `GET /api/system/mode`
  - `POST /api/system/mode`
  - `POST /api/system/emergency-stop`
- Windows local actions are runtime-gated (mode + emergency stop).

## 2) Immediate Stabilization (Phase A)

Goal: Stop user-facing network/server errors and make behavior deterministic.

1. Encoding hardening
- Standardize all API/user-facing text to UTF-8-safe strings.
- Remove legacy mojibake text paths from Rust handlers.

2. Chat fallback hardening
- Keep Tauri `proxy_chat` fallback as default path when direct HTTP fails.
- Surface explicit error cause in UI toast/panel.

3. Runtime control enforcement
- Enforce gating consistently across every OS action entrypoint.
- Add audit logs for blocked actions (`mode`, `emergency_stop`, `feature`).

4. Health diagnostics
- Extend health payload with API readiness and key runtime toggles.
- Add one-click diagnostics action in settings.

## 3) Capability Expansion (Phase B)

Goal: Move from command bot to practical workstation copilot.

1. Deterministic tool layer
- File ops (read/list/search/write with policy).
- App launch/focus/clipboard primitives.
- Browser open/search/extract primitives.

2. Workflow execution model
- Plan preview -> approval -> execution traces.
- Step-level retries with failure classification.
- Saved reusable workflow templates.

3. Work app integrations
- Email summarization pipeline.
- Calendar + task extraction.
- Notion/Excel/Word export adapters.

## 4) Always-on Jarvis Mode (Phase C)

Goal: Context-aware assistant with safe autonomy.

1. Observe/Copilot/Autopilot operating model
- Observe: read-only analysis and suggestions.
- Copilot: semi-auto, user-approved actions.
- Autopilot: policy-constrained auto execution.

2. Continuous context loop
- Event stream aggregation (active app/window/content hints).
- Intent prediction for proactive suggestions.
- Quiet-hours and interruption policies.

3. Safety envelope
- Emergency stop (global hard block).
- Risk-scored action policy.
- Full audit trail + replay.

## 5) Production Readiness Gate (Phase D)

Goal: Ops-grade reliability.

1. Testing
- API integration tests for chat + runtime mode endpoints.
- E2E tests for key user journeys (send prompt, execute action, rollback).

2. Reliability
- Crash recovery for sidecars.
- Backoff/retry for external APIs.
- Circuit breaker on failing integrations.

3. Security
- Secret isolation and rotation path.
- Command denylist + allowlist governance.
- Least-privilege execution profile.

## 6) Current Execution Status

- [Done] Runtime mode API endpoints integrated.
- [Done] Emergency stop logic validated against action execution.
- [Done] Corrupted `handle_chat` block restored to compile-safe path.
- [In Progress] Rebuild natural-language routing quality (Korean/English mixed intents).
- [Next] UI controls for runtime mode + emergency stop in Settings.
