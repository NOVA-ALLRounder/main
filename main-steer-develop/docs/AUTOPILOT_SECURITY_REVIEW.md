# Autopilot Default Security Review

Updated: 2026-02-11

## Scope
- Runtime mode defaults (`core/src/runtime_mode.rs`, `start_steer.ps1`, launcher/settings fallbacks)
- Approval gate behavior (`core/src/approval_gate.rs`)
- Plan execution of shell steps (`core/src/execution_controller.rs`)

## Findings
1. High: Unknown shell commands were implicitly auto-approved.
- Previous behavior: `ApprovalGate::check_command()` defaulted to `AutoApprove`.
- Risk: In autopilot mode, unclassified commands could execute without explicit approval.
- Fix: Default changed to `RequireApproval` for unknown commands.

2. High: `StepType::Shell` path executed commands without approval gate enforcement.
- Previous behavior: shell command executed directly in `execution_controller` without policy check.
- Risk: Approval policy bypass.
- Fix: Added mandatory gate check before execution:
  - `Blocked` -> stop execution with `blocked` status context.
  - `RequireApproval` -> pause with `approval_required`.
  - `AutoApprove` -> execute command.

3. Medium: Legacy approval API always approved.
- Previous behavior: `evaluate_approval()` returned approved unconditionally.
- Risk: Inconsistent policy behavior for approval-driven steps.
- Fix: `evaluate_approval()` now delegates to `preview_approval()`.

## Default Mode Changes
- Default runtime mode changed to `autopilot` when env var is absent.
  - `RuntimeControl::from_env()` fallback set to `Autopilot`.
  - `start_steer.ps1` now sets `STEER_OPERATION_MODE=autopilot` if unset.
  - UI fallback rendering now assumes `autopilot` when runtime mode is temporarily unavailable.

## Remaining Risks / Next Hardening
1. Authentication
- API auth is optional when `STEER_API_KEY` is empty.
- Recommendation: require non-empty API key in production and reject startup otherwise.

2. CORS
- Current CORS is permissive (`Any`) for local runtime compatibility.
- Recommendation: tighten to known local origins in production builds.

3. Allowlist & policy depth
- Pattern-based command checks are useful but incomplete.
- Recommendation: add structured command parser + explicit allowlist for executable roots and arguments.

4. Auditability
- Recommendation: persist all approval/deny events with actor, reason, and command hash for traceability.
