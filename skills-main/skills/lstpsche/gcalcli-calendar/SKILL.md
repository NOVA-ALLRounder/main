---
name: gcalcli-calendar
description: "Google Calendar via gcalcli: today-only agenda by default, bounded meaning-first lookup via agenda scans, and fast create/delete with verification--optimized for low tool calls and minimal output."
metadata: {"openclaw":{"emoji":"📅","requires":{"bins":["gcalcli"]}}}
---

# gcalcli-calendar

Use `gcalcli` to read/search/manage Google Calendar with minimal tool calls and minimal output.

## Rules

### Output & language
- Don't print CLI commands/flags/tool details unless the user explicitly asks (e.g. "show commands used", "/debug", "/commands").
- If asked for commands: print ALL executed commands in order (including retries) and nothing else.
- Don't mix languages within one reply.
- Be concise. No scope unless nothing found.

### Dates & formatting
- Human-friendly dates by default. ISO only if explicitly requested.
- Don't quote event titles unless needed to disambiguate.

### Calendar scope
- Trust gcalcli config (default/ignore calendars). Don't broaden scope unless user asks "across all calendars" or results are clearly wrong.

### Agenda (today-only by default)
- If user asks "agenda" without a period, return today only.
- Expand only if explicitly asked (tomorrow / next N days / date range).

### Weekday requests (no mental math)
If user says "on Monday/Tuesday/..." without a date:
1) fetch next 14 days agenda once,
2) pick matching day/event from tool output,
3) proceed (or disambiguate if multiple).

### Finding events: prefer deterministic agenda scan (meaning-first)
When locating events to cancel/delete/edit:
- Prefer `agenda` over `search`.
- Use a bounded window and match events by meaning (semantic match) rather than exact text.
- Default locate windows:
  - If user gives an exact date: scan that day only.
  - If user gives a weekday: scan next 14 days.
  - If user gives only meaning words ("train", "lecture", etc.) with no date: scan next 30 days first.
  - If still not found: expand to 180 days and say so only if still empty.

Use gcalcli `search` only as a fallback when:
- the time window would be too large to scan via agenda (token-heavy), or
- the user explicitly asked to "search".

### Search (bounded)
- Default search window: next ~180 days (unless user specified otherwise).
- If no matches: say "No matches in next ~6 months (<from>-><to>)" and offer to expand.
- Show scope only when nothing is found.

### Tool efficiency
- Default: use `--nocolor` to reduce formatting noise and tokens.
- Use `--tsv` only if you must parse/dedupe/sort.

## Actions policy (optimized)

### Unambiguous actions run immediately
For cancel/delete/edit actions:
- Do NOT ask for confirmation by default.
- Run immediately ONLY if the target event is unambiguous:
  - single clear match in a tight window, OR
  - user specified exact date+time and a matching event exists.

If ambiguous (multiple candidates):
- Ask a short disambiguation question listing the smallest set of candidates (1-3 lines) and wait.

### Create events: overlap check MUST be cross-calendar (non-ignored scope)
When creating an event:
- Always run a best-effort overlap check across ALL non-ignored calendars by scanning agenda WITHOUT `--calendar`.
  - This ensures overlaps are detected even if the new event is created into a specific calendar.
- If overlap exists with busy events:
  - Ask for confirmation before creating.
- If no overlap:
  - Create immediately.

### Deletes must be reliable
- Use non-interactive delete with `--iamaexpert` (avoid gcalcli prompts).
- Verify once via agenda in the same tight window.
- If verification still shows the event, do one retry with `--refresh`.
- Never claim success unless verification confirms.

## Canonical commands

### Agenda (deterministic listing)
- Today: `gcalcli --nocolor agenda today tomorrow`
- Next 14d (weekday resolution): `gcalcli --nocolor agenda today +14d`
- Next 30d (meaning-first locate): `gcalcli --nocolor agenda today +30d`
- Custom: `gcalcli --nocolor agenda <start> <end>`

### Search (fallback / explicit request)
- Default (~6 months): `gcalcli --nocolor search "<query>" today +180d`
- Custom: `gcalcli --nocolor search "<query>" <start> <end>`

### Create
- Overlap preflight (tight, cross-calendar):
  - `gcalcli --nocolor agenda <start> <end>`
  - IMPORTANT: do NOT add `--calendar` here; overlaps must be checked across all non-ignored calendars.
- Create into a specific calendar:
  - Quick: `gcalcli --nocolor --calendar "<CalendarName>" quick "<event text>"`
  - Add: `gcalcli --nocolor --calendar "<CalendarName>" --title "<Title>" --when "<Start>" --duration <minutes> add`

### Delete (no confirmation if unambiguous)
- Locate via agenda (preferred):
  - `gcalcli --nocolor agenda <dayStart> <dayEnd>` (exact date)
  - `gcalcli --nocolor agenda today +14d` (weekday)
  - `gcalcli --nocolor agenda today +30d` (meaning only)
- Delete (non-interactive, bounded):
  - `gcalcli --nocolor --iamaexpert delete "<query>" <start> <end>`
- Verify (same window):
  - `gcalcli --nocolor agenda <dayStart> <dayEnd>`
- Optional one retry if still present:
  - `gcalcli --nocolor --refresh agenda <dayStart> <dayEnd>`
