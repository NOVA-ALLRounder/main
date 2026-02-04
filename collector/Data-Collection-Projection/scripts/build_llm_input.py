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

    return {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "date_local": daily.get("date_local"),
        "top_apps": top_apps,
        "top_titles": top_titles,
        "key_events": daily.get("key_events", {}),
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
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def _trim_weekday_patterns(value: dict, max_items: int) -> dict:
    trimmed = {}
    for weekday, items in (value or {}).items():
        if not isinstance(items, list):
            continue
        trimmed[weekday] = items[: max(0, max_items)]
    return trimmed


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
    return value[: max_len - 1] + "…"


if __name__ == "__main__":
    main()
