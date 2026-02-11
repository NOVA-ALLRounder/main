# Rollout Checklist

## Pre-Deploy
- [ ] Confirm `steer.db` is backed up.
- [ ] Validate required env vars (`OPENAI_API_KEY`, n8n creds, etc.).
- [ ] Decide if chat gating is required for this deployment.

## Migration
- [ ] Start server once to auto-create new tables (`exec_results`, `quality_scores`).
- [ ] Verify DB schema with a read-only query if needed.

## Runtime Verification
- [ ] Run `POST /api/verify/runtime` on a staging workspace.
- [ ] (Optional) Generate E2E spec and run with `run_e2e=true`.

## Quality Gate
- [ ] Run `POST /api/quality/score` and verify `GET /api/quality/latest`.
- [ ] Check Dashboard shows Quality Score.

## Safety Controls
- [ ] Confirm shell allowlist/denylist are correct.
- [ ] Confirm tool allowlist/denylist matches desired restrictions.
- [ ] Verify chat gate blocks/permits as expected (if enabled).

## Rollback
- [ ] Stop the service and restore the last known-good build.
- [ ] Restore `steer.db` backup if needed.

