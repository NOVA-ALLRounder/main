# Openclaw Reference Notes (Deep Scan Summary)

## Scope
- Repos scanned under `moltbot/openclaw/*` for UI automation / vision / planner / recovery patterns.
- Goal: find reusable reference patterns for improving local-os-agent scenario success rate.

## High-Value References

### 1) Lobster workflow execution + resume/approval
**Paths**
- `moltbot/openclaw/lobster/src/workflows/file.ts`
- `moltbot/openclaw/lobster/VISION.md`

**Key ideas**
- Workflows are file-based, resumable via tokens (resume state stored + resumed safely).
- `approve` is a hard stop; execution only proceeds after explicit resume.
- Designed for deterministic multi-step execution with state tracking.

**Why it matters**
- Our surf runs fail at step boundaries (LLM refused / snapshot loops). A resumable workflow layer could re-enter cleanly without full replanning.

---

### 2) LLM invocation with retry context + validation
**Path**
- `moltbot/openclaw/lobster/src/commands/stdlib/llm_task_invoke.ts`

**Key ideas**
- `retryContext` includes `attempt` and `validationErrors` when retrying.
- Bounded retries when schema validation fails.
- Run-state + cache to avoid duplicate LLM calls.

**Why it matters**
- We see repeated LLM refusals and plan loops. Passing a structured retry context (and optionally validation errors) can nudge LLMs to take a different action.
- Run-state / cache could prevent repeated planning on identical screenshots.

---

## What *Not* Found
- No direct UI automation engine (click/vision) implementations in Openclaw repos.
- No explicit "planner" or "vision" logic beyond workflow concepts.

---

## Actionable Mapping to local-os-agent

### A) Add retry context to planner
- When a plan is refused or repeats, inject `{attempt, last_action, error_summary}` into the prompt.
- Similar to Lobster’s `retryContext`.

### B) Add resumable workflow step tracking
- Record step boundaries with small checkpoints.
- Allow resume without full replan when a step is deterministic (e.g., Cmd+N → type → Cmd+A → Cmd+C).

### C) Cache stable planning outputs
- If the screenshot hash + goal is unchanged, reuse last plan to avoid repeated LLM calls.

---

## References Index
- `moltbot/openclaw/lobster/src/workflows/file.ts`
- `moltbot/openclaw/lobster/src/commands/stdlib/llm_task_invoke.ts`
- `moltbot/openclaw/lobster/VISION.md`


## Additional Recommendations (from notes)

### 1) 강화된 실패 복구 루틴
- 실패 원인별 회복 루틴 분기 (예: Mail/Notes 포커스 재확인 → Cmd+N → 타이핑).
- 특정 앱에서 “항상 포커스 확보 → 첫 입력” 보장.

### 2) 스냅샷 루프 차단 + 명확한 다음 행동 강제
- `snapshot`가 2번 연속 나오면 강제로 “다음 액션 후보 리스트” 중 하나 선택.
- 목표 기반 규칙 자동 삽입 (예: Mail이면 compose 창 없으면 Cmd+N).

### 3) 앱별 최소 AppleScript 유틸 추가
- 하드코딩 시나리오가 아니라 앱별 안전 루틴으로 범용 적용.
- 예: Mail이면 “새 메일 창 없으면 Cmd+N → 본문 클릭 → 제목 입력”.

### 4) Vision 의존 낮추기
- 검색/텍스트 선택처럼 Vision 불필요한 단계는 키보드 중심 경로로 진행.
- Vision은 “읽기/클릭”이 정말 필요한 순간에만 사용.

### 5) 프롬프트를 “행동 우선”으로 재정비
- snapshot 반복으로 행동 못하는 상황 방지.
- “행동 없으면 실패” 규칙과 강제 fallback 분기 추가.

### 추천 조합
- 2 + 3 + 4 조합이 가장 범용적이고 하드코딩 느낌 없이 성공률 상승 가능.
- 진행 방향 선택지:
  1) 범용 복구 루틴 강화 (추천)
  2) 프롬프트/정책 튜닝 중심
  3) 둘 다 진행 (가장 안정적)

## Further Ideas Found in Openclaw (Additional Deep Scan)

### 6) Peekaboo CLI patterns (UI automation best practices)
**Path**: `moltbot/openclaw/nix-steipete-tools/tools/peekaboo/skills/peekaboo/SKILL.md`

**Key ideas**
- Built-in permission checks: `peekaboo permissions` (explicit Screen Recording + Accessibility verification).
- Focus retry & timeout flags (`--focus-timeout-seconds`, `--focus-retry-count`).
- Snapshot cache reuse; `see` produces element IDs for deterministic clicking.
- Menu/dock/menubar actions + app/window focus utilities.

**Actionable mapping**
- Add explicit permission verification step (screen/accessibility) before starting surf.
- Add focus retry loops (with bounded retries) when switching apps or typing.
- Implement a snapshot-cache-like mechanism: if the UI map is unchanged, reuse target IDs or last-known coordinates rather than re-planning.

---

### 7) Session reattach + duplicate prompt guard (Oracle skill)
**Path**: `moltbot/openclaw/nix-steipete-tools/tools/oracle/skills/oracle/SKILL.md`

**Key ideas**
- Long runs should “reattach” rather than re-run on timeout.
- Duplicate prompt guard: avoid re-sending identical prompts unless forced.

**Actionable mapping**
- If a surf run times out or errors, resume last session state instead of re-planning from scratch.
- Add duplicate-plan guard: if goal + screenshot hash unchanged, avoid re-issuing the same plan.

---

### 8) Config-level safeguard/cache concepts (Nix Openclaw)
**Path**: `moltbot/openclaw/nix-openclaw/nix/generated/openclaw-config-options.nix`

**Key ideas**
- `compaction.mode` includes `safeguard`.
- `contextPruning.mode` supports `cache-ttl`.
- Numerous timeout/retry options in config.

**Actionable mapping**
- Introduce a “safeguard” planning mode: reduce risk by preferring deterministic keyboard paths.
- Add cache TTL to planning history to avoid stale loops.
- Enforce explicit timeouts per step and a retry budget for UI focus/LLM failures.

---

## Additional Findings from Clawdbot/OpenClaw (Deep Scan)

### 9) TCC permission stability + recovery (macOS)
**Paths**
- `clawdbot-main/docs/platforms/mac/permissions.md`
- `moltbot/openclaw/openclaw/docs/platforms/mac/permissions.md`
- `moltbot/openclaw/openclaw/docs/platforms/mac/signing.md`

**Key ideas**
- TCC grants bind to *app path + bundle ID + code signature*.
- Ad‑hoc signatures are fragile; prompts can disappear unless the signed app identity stays stable.
- Recovery checklist: remove app in Privacy & Security, relaunch from fixed path, and `tccutil reset` for Accessibility/ScreenCapture/AppleEvents.

**Actionable mapping**
- Add explicit preflight checks for Screen Recording + Accessibility and return clear instructions.
- Provide “permission reset” guidance when checks fail.
- Avoid moving executables or changing bundle IDs between runs (if/when we ship a GUI wrapper).

---

### 10) Peekaboo permissions + focus retries (CLI best practices)
**Paths**
- `moltbot/openclaw/nix-steipete-tools/tools/peekaboo/skills/peekaboo/SKILL.md`
- `moltbot/openclaw/openclaw/docs/platforms/mac/peekaboo.md`

**Key ideas**
- `peekaboo permissions` provides direct Screen Recording / Accessibility status.
- CLI supports focus retries (`--focus-retry-count`, `--focus-timeout-seconds`) on interaction commands.
- Snapshot cache is short‑lived; re‑snapshot when refs feel stale.

**Actionable mapping**
- Add peekaboo permission preflight, fail fast with instructions.
- Add focus retry flags for `click` operations (and retry app focus before typing).
- Treat snapshot refs as ephemeral (use new snapshot after navigation).

---

### 11) Browser snapshot + ref stability (Web automation)
**Paths**
- `clawdbot-main/docs/tools/browser.md`
- `moltbot/openclaw/openclaw/docs/tools/browser.md`

**Key ideas**
- Two snapshot styles: AI snapshot (numeric refs) vs role snapshot (`e12`).
- Refs are *not stable across navigations*; always re‑snapshot after URL change.
- Optional `--labels` mode to attach screenshot with ref overlays.

**Actionable mapping**
- Invalidate cached refs on `open_url`/navigation.
- Prefer snapshot → ref click flow over blind `click_visual`.
- Use ref overlays/screenshot for debugging when click targets are ambiguous.

---

### 12) Agent loop retry semantics (don’t redo finished steps)
**Paths**
- `moltbot/openclaw/openclaw/docs/concepts/agent-loop.md`
- `moltbot/openclaw/openclaw/docs/concepts/retry.md`

**Key ideas**
- Retry only the *current* step; don’t repeat completed steps.
- On retry, reset in‑memory buffers to avoid duplicate output.

**Actionable mapping**
- Add step‑level retry budget and avoid re‑running finished steps.
- Keep a lightweight “last successful step” checkpoint to resume safely.
