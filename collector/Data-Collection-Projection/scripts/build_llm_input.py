from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build LLM input dataset")
    parser.add_argument("--daily", default="", help="path to daily_summary.json")
    parser.add_argument("--pattern", default="", help="path to pattern_summary.json")
    parser.add_argument("--output", default="llm_input.json", help="output path")
    parser.add_argument("--max-top-apps", type=int, default=5)
    parser.add_argument("--max-patterns", type=int, default=8)
    parser.add_argument("--max-titles", type=int, default=5)
    parser.add_argument("--max-title-len", type=int, default=60)
    parser.add_argument("--max-weekday-patterns", type=int, default=5)
    parser.add_argument("--max-sequences", type=int, default=5)
    parser.add_argument("--max-transitions", type=int, default=5)
    parser.add_argument("--max-bytes", type=int, default=8000)
    parser.add_argument("--no-redact-sensitive", action="store_false", dest="redact_sensitive")
    parser.add_argument(
        "--config",
        default="",
        help="optional config path for DB storage",
    )
    parser.add_argument(
        "--include-apps",
        default="",
        help="comma-separated app allowlist for LLM input",
    )
    parser.add_argument(
        "--hours",
        default="",
        help="hour filter, e.g. 9-18 or 9,10,11",
    )
    parser.add_argument("--store-db", action="store_true", help="store input in DB")
    parser.set_defaults(redact_sensitive=True)
    return parser.parse_args()


def _load_json(path: str) -> dict:
    if not path:
        return {}
    p = Path(path)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def main() -> None:
    args = parse_args()
    daily = _load_json(args.daily)
    pattern = _load_json(args.pattern)

    include_apps = _parse_apps(args.include_apps)
    include_hours = _parse_hours(args.hours)

    output = _build_payload(
        daily,
        pattern,
        max_top_apps=args.max_top_apps,
        max_patterns=args.max_patterns,
        max_titles=args.max_titles,
        max_title_len=args.max_title_len,
        max_weekday_patterns=args.max_weekday_patterns,
        max_sequences=args.max_sequences,
        max_transitions=args.max_transitions,
        include_apps=include_apps,
        include_hours=include_hours,
        redact_sensitive=args.redact_sensitive,
    )

    output = _compress_payload(output, max_bytes=args.max_bytes)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"llm_input_saved={out_path}")

    if args.store_db:
        created_at = output.get("generated_at") or datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        payload_json = json.dumps(output, ensure_ascii=False, separators=(",", ":"))
        payload_size = len(payload_json.encode("utf-8"))
        _store_llm_input(args, created_at, payload_json, payload_size)


def _build_payload(
    daily: dict,
    pattern: dict,
    *,
    max_top_apps: int,
    max_patterns: int,
    max_titles: int,
    max_title_len: int,
    max_weekday_patterns: int,
    max_sequences: int,
    max_transitions: int,
    include_apps: set[str],
    include_hours: set[int],
    redact_sensitive: bool,
) -> dict:
    top_apps = daily.get("top_apps") or []
    if include_apps:
        top_apps = [item for item in top_apps if item.get("app") in include_apps]
    top_apps = top_apps[: max_top_apps]

    top_titles = daily.get("top_titles") or []
    if include_apps:
        top_titles = [item for item in top_titles if item.get("app") in include_apps]
    sanitized_titles = []
    for item in top_titles:
        title = str(item.get("title_hint") or "").strip()
        if not title:
            continue
        title = _normalize_title(str(item.get("app") or ""), title)
        if redact_sensitive and _contains_sensitive(title):
            continue
        title = _trim_title(title, max_title_len)
        sanitized_titles.append({**item, "title_hint": title})
    top_titles = sanitized_titles[: max_titles]

    hourly_patterns = pattern.get("patterns") or []
    if include_hours:
        hourly_patterns = [
            item for item in hourly_patterns if int(item.get("hour", -1)) in include_hours
        ]
    if include_apps:
        hourly_patterns = [
            item for item in hourly_patterns if item.get("app") in include_apps
        ]
    hourly_patterns = hourly_patterns[: max_patterns]

    weekday_patterns = _trim_weekday_patterns(
        pattern.get("weekday_patterns") or {}, max_weekday_patterns
    )
    if include_hours or include_apps:
        weekday_patterns = _filter_weekday_patterns(
            weekday_patterns, include_apps, include_hours
        )

    sequence_patterns = pattern.get("sequence_patterns") or []
    if include_apps:
        sequence_patterns = [
            item
            for item in sequence_patterns
            if any(app in include_apps for app in item.get("sequence", []))
        ]
    sequence_patterns = sequence_patterns[: max_sequences]

    transition_patterns = pattern.get("transition_patterns") or []
    if include_apps:
        transition_patterns = [
            item
            for item in transition_patterns
            if item.get("from") in include_apps or item.get("to") in include_apps
        ]
    transition_patterns = transition_patterns[: max_transitions]

    time_bucket_patterns = pattern.get("time_bucket_patterns") or {}
    focus_block_stats = pattern.get("focus_block_stats") or {}

    counts = daily.get("counts") or {}
    top_transitions = daily.get("top_transitions") or []
    key_event_tokens = []
    for event_name in (daily.get("key_events") or {}).keys():
        key_event_tokens.extend(_tokenize_event(event_name))
    key_event_tokens = _unique_limited(key_event_tokens, 12)

    intents = _infer_intents(top_apps, key_event_tokens, top_transitions)
    workflow_hints = _workflow_hints(intents)
    if top_transitions:
        workflow_hints = dict(workflow_hints or {})
        workflow_hints["recommended_template"] = "sequence_triggered_notification"
        if not workflow_hints.get("required_tools"):
            workflow_hints["required_tools"] = ["slack"]
        workflow_hints["sequence_based"] = True

    task_units = _infer_task_units(top_apps, top_titles, key_event_tokens)
    task_summary = (
        {
            "primary_task": task_units[0]["unit_id"],
            "label": task_units[0]["label"],
            "confidence": task_units[0]["confidence"],
        }
        if task_units
        else {}
    )

    repetition = _pattern_repetition(sequence_patterns, transition_patterns)
    auto_approve = bool(repetition.get("sequence_support", 0) >= 2 or repetition.get("transition_support", 0) >= 2)
    if auto_approve:
        workflow_hints = dict(workflow_hints or {})
        workflow_hints["auto_approve"] = True

    return {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "date_local": daily.get("date_local"),
        "counts": counts,
        "top_apps": top_apps,
        "top_titles": top_titles,
        "time_context": _time_context(daily.get("date_local"), hourly_patterns),
        "key_events": daily.get("key_events", {}),
        "key_event_tokens": key_event_tokens,
        "app_transitions": top_transitions,
        "intent_candidates": intents,
        "intent_summary": (
            {
                "primary_intent": intents[0]["intent_id"],
                "confidence": intents[0]["confidence"],
                "label": intents[0]["label"],
            }
            if intents
            else {}
        ),
        "workflow_hints": workflow_hints,
        "sequence_signature": _sequence_signature_from_transitions(top_transitions),
        "task_units": task_units,
        "task_summary": task_summary,
        "pattern_repetition": repetition,
        "auto_approve": auto_approve,
        "hourly_patterns": hourly_patterns,
        "weekday_patterns": weekday_patterns,
        "sequence_patterns": sequence_patterns,
        "transition_patterns": transition_patterns,
        "time_bucket_patterns": time_bucket_patterns,
        "focus_block_stats": focus_block_stats,
        "notes": [
            "Use hourly_patterns to infer likely activities at specific times.",
            "top_titles are masked/normalized hints, not raw content.",
        ],
    }

def _compress_payload(payload: dict, *, max_bytes: int) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(value: dict) -> int:
        return len(json.dumps(value, ensure_ascii=False).encode("utf-8"))

    if _size(payload) <= max_bytes:
        return payload

    compact = dict(payload)
    compact["top_titles"] = []
    if _size(compact) <= max_bytes:
        return compact

    compact["app_transitions"] = (compact.get("app_transitions") or [])[:4]
    compact["key_event_tokens"] = (compact.get("key_event_tokens") or [])[:8]
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = (compact.get("top_apps") or [])[:3]
    compact["hourly_patterns"] = (compact.get("hourly_patterns") or [])[:5]
    compact["weekday_patterns"] = _trim_weekday_patterns(
        compact.get("weekday_patterns") or {}, 3
    )
    compact["sequence_patterns"] = (compact.get("sequence_patterns") or [])[:3]
    compact["transition_patterns"] = (compact.get("transition_patterns") or [])[:3]
    compact["focus_block_stats"] = compact.get("focus_block_stats") or {}
    if _size(compact) <= max_bytes:
        return compact

    compact["hourly_patterns"] = (compact.get("hourly_patterns") or [])[:3]
    compact["transition_patterns"] = []
    compact["time_bucket_patterns"] = {}
    compact["focus_block_stats"] = {}
    compact["key_events"] = {}
    compact["key_event_tokens"] = []
    compact["app_transitions"] = []
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def _trim_weekday_patterns(value: dict, max_items: int) -> dict:
    trimmed = {}
    for weekday, items in (value or {}).items():
        if not isinstance(items, list):
            continue
        trimmed[weekday] = items[: max(0, max_items)]
    return trimmed


def _tokenize_event(name: str) -> list[str]:
    import re

    tokens = re.split(r"[^A-Za-z0-9]+", name or "")
    stop = {"os", "app", "focus", "block", "idle", "event", "window"}
    out = []
    for token in tokens:
        if not token:
            continue
        t = token.lower()
        if t in stop or len(t) <= 2:
            continue
        out.append(t)
    return out


def _unique_limited(values: list[str], limit: int) -> list[str]:
    seen = set()
    out = []
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        out.append(value)
        if len(out) >= limit:
            break
    return out


def _normalize_app_name(name: str) -> str:
    return str(name or "").strip().lower()


def _normalize_title(app: str, title: str) -> str:
    app_key = (app or "").lower()
    value = title.strip()
    suffix_map = {
        "chrome.exe": [" - Google Chrome", " - Chrome"],
        "msedge.exe": [" - Microsoft Edge", " - Edge"],
        "whale.exe": [" - Whale", " - Naver Whale"],
        "notion.exe": [" - Notion"],
        "code.exe": [
            " - Visual Studio Code",
            " - Visual Studio Code Insiders",
            " - Code",
        ],
    }
    for suffix in suffix_map.get(app_key, []):
        if value.endswith(suffix):
            value = value[: -len(suffix)].strip()
            break
    return value


def _infer_task_units(
    top_apps: list[dict],
    top_titles: list[dict],
    key_event_tokens: list[str],
) -> list[dict]:
    app_names = {_normalize_app_name(item.get("app")) for item in top_apps if item.get("app")}
    title_blob = " ".join([str(item.get("title_hint", "")).lower() for item in top_titles if item.get("title_hint")])

    def add_unit(units: list[dict], unit_id: str, label: str, evidence: list[str], base: float = 0.6) -> None:
        confidence = min(0.95, base + 0.1 * len(evidence))
        units.append(
            {
                "unit_id": unit_id,
                "label": label,
                "confidence": round(confidence, 2),
                "evidence": evidence,
            }
        )

    units: list[dict] = []
    tokens = [t.lower() for t in key_event_tokens]

    evidence = []
    if any(app in app_names for app in {"notion.exe"}):
        evidence.append("app:notion")
    if "doc" in title_blob or "note" in title_blob:
        evidence.append("title:doc")
    if evidence:
        add_unit(units, "doc_write", "Writing/Documentation", evidence)

    evidence = []
    if any(app in app_names for app in {"code.exe", "pycharm.exe", "idea.exe"}):
        evidence.append("app:code")
    if evidence:
        add_unit(units, "coding", "Coding/Development", evidence)

    evidence = []
    if any(app in app_names for app in {"slack.exe", "kakaotalk.exe"}):
        evidence.append("app:chat")
    if "message" in tokens or "chat" in tokens:
        evidence.append("event:message")
    if evidence:
        add_unit(units, "communication", "Communication", evidence)

    evidence = []
    if "email" in tokens or "mail" in tokens:
        evidence.append("event:mail")
    if evidence:
        add_unit(units, "email", "Email/Follow-up", evidence)

    evidence = []
    if "task" in tokens or "jira" in tokens:
        evidence.append("event:task")
    if evidence:
        add_unit(units, "task_management", "Task Management", evidence)

    units.sort(key=lambda item: item.get("confidence", 0), reverse=True)
    return units[:4]


def _time_context(date_local: str | None, hourly_patterns: list[dict]) -> dict:
    weekday = ""
    if date_local:
        try:
            weekday = datetime.strptime(date_local, "%Y-%m-%d").strftime("%a")
        except Exception:
            weekday = ""
    dominant_hour = ""
    if hourly_patterns:
        dominant_hour = str(hourly_patterns[0].get("hour", ""))
    return {"weekday_local": weekday, "dominant_hour": dominant_hour}


def _pattern_repetition(sequence_patterns: list[dict], transition_patterns: list[dict]) -> dict:
    seq_support = 0
    seq_conf = 0.0
    if sequence_patterns:
        seq_support = int(sequence_patterns[0].get("support", 0))
        seq_conf = float(sequence_patterns[0].get("confidence", 0.0))
    trans_support = 0
    if transition_patterns:
        trans_support = int(transition_patterns[0].get("support", 0))
    return {
        "sequence_support": seq_support,
        "sequence_confidence": round(seq_conf, 2),
        "transition_support": trans_support,
    }


def _infer_intents(
    top_apps: list[dict],
    key_event_tokens: list[str],
    transitions: list[dict],
) -> list[dict]:
    app_names = {_normalize_app_name(item.get("app")) for item in top_apps if item.get("app")}

    def _hit(token_list: list[str], *needles: str) -> bool:
        for needle in needles:
            if needle in token_list:
                return True
        return False

    intents: list[dict] = []

    def add_intent(intent_id: str, label: str, evidence: list[str], base: float = 0.6) -> None:
        confidence = min(0.95, base + 0.1 * len(evidence))
        intents.append(
            {
                "intent_id": intent_id,
                "label": label,
                "confidence": round(confidence, 2),
                "evidence": evidence,
            }
        )

    tokens = [t.lower() for t in key_event_tokens]
    evidence: list[str] = []
    if _hit(tokens, "summary", "report", "daily"):
        evidence.append("key_event_tokens")
    if "notion" in " ".join(app_names):
        evidence.append("app:notion")
    if evidence:
        add_intent("daily_summary", "Daily summary/report", evidence)

    evidence = []
    if _hit(tokens, "slack", "notify", "message"):
        evidence.append("key_event_tokens")
    if any("slack" in app for app in app_names):
        evidence.append("app:slack")
    if evidence:
        add_intent("team_update", "Team update/notification", evidence)

    evidence = []
    if _hit(tokens, "email", "gmail", "outlook", "followup"):
        evidence.append("key_event_tokens")
    if any("outlook" in app or "gmail" in app for app in app_names):
        evidence.append("app:mail")
    if evidence:
        add_intent("followup_email", "Draft follow-up email", evidence)

    evidence = []
    if _hit(tokens, "file", "save", "export", "upload"):
        evidence.append("key_event_tokens")
    if evidence:
        add_intent("file_backup", "File save/export follow-up", evidence)

    if transitions:
        add_intent(
            "focus_recap",
            "Focus recap from app switching",
            ["app_transitions"],
            base=0.5,
        )

    intents.sort(key=lambda item: item.get("confidence", 0), reverse=True)
    return intents[:3]


def _workflow_hints(intents: list[dict]) -> dict:
    if not intents:
        return {}
    primary = intents[0]["intent_id"]
    template_map = {
        "daily_summary": "daily_summary_to_notion",
        "team_update": "focus_report_to_slack",
        "followup_email": "followup_email_draft",
        "file_backup": "file_save_followup",
        "focus_recap": "focus_report_to_slack",
    }
    required_tools_map = {
        "daily_summary": ["notion"],
        "team_update": ["slack"],
        "followup_email": ["gmail"],
        "file_backup": ["notion"],
        "focus_recap": ["slack"],
    }
    return {
        "recommended_template": template_map.get(primary, ""),
        "required_tools": required_tools_map.get(primary, []),
    }


def _sequence_signature_from_transitions(transitions: list[dict]) -> str:
    if not transitions:
        return ""
    # Build a coarse signature from top transitions.
    seq = []
    for item in transitions[:3]:
        app_from = item.get("from")
        app_to = item.get("to")
        if app_from:
            seq.append(str(app_from))
        if app_to:
            seq.append(str(app_to))
    if not seq:
        return ""
    # De-dup sequential repeats.
    compact = []
    for app in seq:
        if not compact or compact[-1] != app:
            compact.append(app)
    return " -> ".join(compact[:6])


def _store_llm_input(
    args: argparse.Namespace, created_at: str, payload_json: str, payload_size: int
) -> None:
    try:
        # lazy import to avoid heavy dependency in normal path
        import sys
        PROJECT_ROOT = Path(__file__).resolve().parents[1]
        sys.path.insert(0, str(PROJECT_ROOT / "src"))
        from collector.config import load_config
    except Exception:
        return
    if not args.config:
        return
    config = load_config(Path(args.config))
    import sqlite3

    summary_db_path = config.summary_db_path or config.db_path
    conn = sqlite3.connect(str(summary_db_path))
    cur = conn.cursor()
    _ensure_summary_tables(cur)
    cur.execute(
        """
        INSERT INTO llm_inputs (created_at, payload_json, payload_size)
        VALUES (?, ?, ?)
        """,
        (created_at, payload_json, payload_size),
    )
    conn.commit()
    conn.close()


def _ensure_summary_tables(cur) -> None:
    migrations_path = Path(__file__).resolve().parents[1] / "migrations" / "007_summaries.sql"
    if migrations_path.exists():
        cur.executescript(migrations_path.read_text(encoding="utf-8"))


def _filter_weekday_patterns(
    value: dict, include_apps: set[str], include_hours: set[int]
) -> dict:
    filtered = {}
    for weekday, items in (value or {}).items():
        if not isinstance(items, list):
            continue
        current = items
        if include_hours:
            current = [
                item for item in current if int(item.get("hour", -1)) in include_hours
            ]
        if include_apps:
            current = [item for item in current if item.get("app") in include_apps]
        filtered[weekday] = current
    return filtered


def _parse_apps(value: str) -> set[str]:
    if not value:
        return set()
    return {item.strip() for item in value.split(",") if item.strip()}


def _parse_hours(value: str) -> set[int]:
    if not value:
        return set()
    value = value.strip()
    hours = set()
    if "-" in value:
        start_s, end_s = value.split("-", 1)
        try:
            start = int(start_s)
            end = int(end_s)
            for hour in range(start, end + 1):
                hours.add(hour)
        except ValueError:
            return set()
    else:
        for part in value.split(","):
            try:
                hours.add(int(part.strip()))
            except ValueError:
                continue
    return hours


def _contains_sensitive(value: str) -> bool:
    import re

    if re.search(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}", value):
        return True
    if re.search(r"[A-Za-z]:\\\\|/Users/|/home/|/Volumes/", value):
        return True
    if re.search(r"https?://", value):
        return True
    if re.search(r"\\b\\d{12,}\\b", value):
        return True
    return False


def _trim_title(value: str, max_len: int) -> str:
    if max_len <= 0:
        return value
    if len(value) <= max_len:
        return value
    return value[: max_len - 1] + "?"


if __name__ == "__main__":
    main()
