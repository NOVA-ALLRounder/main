# Proactive Workflow Stability Plan

Updated: 2026-02-11
Scope: "user activity logs -> LLM proposes workflows -> n8n workflow auto-generation"

## Reference Scenario (User Target)

Primary scenario to optimize for:

1. Check business email (internal/personal work inbox).
2. Summarize key items and action points.
3. Execute follow-up work using productivity tools when needed:
   - Excel
   - Notion
   - Word
4. Produce clear output artifacts and status updates.

This scenario is the baseline acceptance target for proactive workflow stability.

## Jarvis-Style Always-On Direction

User-intended product direction:

- Agent runs continuously in the background.
- Agent watches screen/app context and keeps understanding workflow context over time.
- Agent proactively suggests next actions.
- User is open to direct mouse/keyboard control by agent for task execution.

To implement this safely, use explicit operation modes.

### Operation Modes

1. Observe mode (always-on safe default)
- Collect context only (active app, event stream, screen metadata).
- No click/type/command execution.
- Proactive suggestions only.

2. Copilot mode
- Agent can prepare actions and execute only after user confirmation.
- Suitable for semi-automated office workflows.

3. Autopilot mode
- Agent can execute approved workflow classes directly (including mouse/keyboard actions).
- Must be bounded by strict policy and safety controls.

## Mandatory Safety Controls for Mouse/Keyboard Automation

Required before enabling broad Autopilot behavior:

1. Global emergency stop hotkey
- Example: immediate kill switch (`Ctrl+Alt+Pause`) to stop all automation.

2. Protected context denylist
- Never automate in sensitive contexts (banking, password dialogs, private messaging windows).

3. High-risk command deny policy
- Block destructive shell/system commands by default.

4. Full action audit trail
- Record each automation action: target app/window, action type, timestamp, result.

5. Mode visibility
- UI must always show current mode (`Observe` / `Copilot` / `Autopilot`) and active automation state.

## Goal

Run proactive workflow suggestion and n8n generation reliably in daily use without frequent `rate_limit`, false-positive recommendations, or broken workflow JSON.

## Current State (as-is)

- Event logging exists and data is stored.
- Pattern analysis and recommendation generation paths exist.
- Recommendation approval path can generate/fix n8n workflow JSON and create workflows in n8n.
- For stability, background analyzers are currently disabled by default in startup:
  - `STEER_DISABLE_ANALYZER=1`
  - `STEER_DISABLE_BACKGROUND_ANALYSIS=1`

## Stability Strategy

Use a phased rollout with hard gates. Do not enable full background mode at once.

## Phase 1: Safe Baseline (Now)

Objective: Keep chat/API stable while preserving manual analysis capability.

- Keep both analyzer flags disabled by default.
- Trigger recommendation generation only on explicit command:
  - `analyze_patterns`
- Ensure all failures are observable:
  - check `core_sidecar.err.log`
  - watch `/api/system/health`

Exit criteria:
- Chat errors do not regress.
- No repeated 429 bursts in normal usage.

## Phase 2: Controlled Re-enable

Objective: Re-enable proactive analysis with strict throttling.

### Required config changes

1. Add runtime config (env-based) for analyzer cadence and limits:
- `STEER_ANALYZER_INTERVAL_SECS` (default: `900`)
- `STEER_ANALYZER_MAX_PATTERNS_PER_RUN` (default: `2`)
- `STEER_ANALYZER_MIN_OCCURRENCES` (default: `5`)
- `STEER_ANALYZER_MIN_SIMILARITY` (default: `0.90`)
- `STEER_ANALYZER_COOLDOWN_SECS` (default: `3600`) per pattern key

2. Add LLM token guard:
- skip recommendation call if recent TPM budget is near threshold
- backoff with jitter after any `rate_limit_exceeded`

3. Add recommendation dedupe:
- do not insert same pattern title/hash within cooldown window

Exit criteria:
- 24h run with no sustained 429 loop.
- Recommendation volume remains bounded and relevant.

## Phase 3: n8n Generation Reliability

Objective: Raise success rate of generated workflows.

### Guardrails

1. Credential-aware prompting is mandatory:
- include only existing credential IDs from n8n
- if missing credentials, generate workflow with clear placeholders/comments

2. Multi-pass JSON validation:
- parse JSON
- validate required node structure
- validate links/edges
- enforce inactive creation on first creation

3. Retry policy:
- max 3 retries with reasoned repair prompt
- persist last failure reason in DB for diagnostics

4. Execution safety:
- keep new workflow inactive by default
- require explicit user enablement

Exit criteria:
- n8n creation success >= 90% on approved recommendations.
- invalid JSON incidence < 5%.

## Phase 4: Production Ops Readiness

Objective: Make behavior measurable and maintainable.

### Observability

Track at least:
- recommendation generation attempts/success/failure
- LLM call count, rate-limit count, retry count
- n8n create success/failure
- median generation latency

Expose via:
- DB metrics table
- dashboard card/API endpoint

### Reliability controls

- Circuit breaker:
  - if repeated 429 or repeated generation failure, auto-disable analyzer for cooldown period
- Feature flags:
  - `STEER_DISABLE_ANALYZER`
  - `STEER_DISABLE_BACKGROUND_ANALYSIS`
  - `STEER_PROACTIVE_MODE` (`off|manual|controlled|full`)

Exit criteria:
- system self-recovers from transient API failures without operator action.

## Recommended Default Modes

- Dev/Local default:
  - `STEER_PROACTIVE_MODE=manual`
  - analyzer disabled, run only explicit command

- Beta default:
  - `STEER_PROACTIVE_MODE=controlled`
  - long interval + strong filtering + cooldown

- Full mode:
  - only after metrics pass and failure rates stay low for 7+ days

## Immediate Next Actions (implementation order)

1. Add analyzer throttle env controls and cooldown logic.
2. Add dedupe key for recommendations (pattern hash + time window).
3. Add rate-limit aware backoff/circuit breaker.
4. Add n8n JSON validation stage before create API call.
5. Add metrics endpoint/dashboard counters for proactive pipeline.

## Rollback Plan

If instability appears:

1. Set:
   - `STEER_DISABLE_ANALYZER=1`
   - `STEER_DISABLE_BACKGROUND_ANALYSIS=1`
2. Restart with `start_steer.ps1 -SkipDcp`.
3. Keep manual `analyze_patterns` only until failure cause is resolved.
