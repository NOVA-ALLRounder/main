from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build realtime LLM input from recent events")
    parser.add_argument("--config", default="", help="optional config path")
    parser.add_argument("--db", default="", help="override db path")
    parser.add_argument("--since-minutes", type=int, default=10)
    parser.add_argument("--output", default="logs/run4/llm_input_realtime.json")
    parser.add_argument("--max-bytes", type=int, default=8000)
    parser.add_argument("--max-top-apps", type=int, default=5)
    parser.add_argument("--max-titles", type=int, default=5)
    parser.add_argument("--max-events", type=int, default=8)
    return parser.parse_args()


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _format_ts(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


def _load_db_path(args: argparse.Namespace) -> Path:
    if args.db:
        return Path(args.db)
    if args.config:
        try:
            import sys

            project_root = Path(__file__).resolve().parents[1]
            sys.path.insert(0, str(project_root / "src"))
            from collector.config import load_config

            config = load_config(Path(args.config))
            return Path(config.db_path)
        except Exception:
            pass
    return Path("collector.db")


def _safe_json(raw: str) -> dict:
    if not raw:
        return {}
    try:
        return json.loads(raw)
    except Exception:
        return {}


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


def _compress_payload(payload: dict, max_bytes: int) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(val: dict) -> int:
        return len(json.dumps(val, ensure_ascii=False).encode("utf-8"))

    if _size(payload) <= max_bytes:
        return payload

    compact = dict(payload)
    compact["top_titles"] = []
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = (compact.get("top_apps") or [])[:3]
    compact["key_events"] = dict(list((compact.get("key_events") or {}).items())[:5])
    if _size(compact) <= max_bytes:
        return compact

    compact["key_events"] = {}
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def main() -> None:
    args = parse_args()
    db_path = _load_db_path(args)
    since_minutes = max(1, int(args.since_minutes))

    end_ts = _now_utc()
    start_ts = end_ts - timedelta(minutes=since_minutes)

    conn = sqlite3.connect(str(db_path))
    cur = conn.cursor()
    rows = cur.execute(
        "SELECT ts, app, event_type, payload_json FROM events WHERE ts >= ? ORDER BY ts ASC",
        (_format_ts(start_ts),),
    ).fetchall()
    conn.close()

    app_durations: dict[str, float] = defaultdict(float)
    app_counts: Counter[str] = Counter()
    title_counts: Counter[tuple[str, str]] = Counter()
    event_counts: Counter[str] = Counter()

    for ts, app, event_type, payload_json in rows:
        app = str(app or "unknown")
        app_counts[app] += 1
        event_counts[str(event_type or "unknown")] += 1
        payload = _safe_json(payload_json)
        duration = payload.get("duration_sec")
        if isinstance(duration, (int, float)):
            app_durations[app] += float(duration)
        title = str(payload.get("window_title") or "").strip()
        if title and not _contains_sensitive(title):
            title = _trim_title(title, 60)
            title_counts[(app, title)] += 1

    top_apps = []
    if app_durations:
        for app, total_sec in sorted(app_durations.items(), key=lambda i: i[1], reverse=True)[: args.max_top_apps]:
            top_apps.append({"app": app, "minutes": round(total_sec / 60.0, 2), "events": app_counts[app]})
    else:
        for app, cnt in app_counts.most_common(args.max_top_apps):
            top_apps.append({"app": app, "events": cnt})

    top_titles = []
    for (app, title), cnt in title_counts.most_common(args.max_titles):
        top_titles.append({"app": app, "title_hint": title, "events": cnt})

    key_events = dict(event_counts.most_common(args.max_events))

    payload = {
        "generated_at": _format_ts(end_ts),
        "window": {"start_utc": _format_ts(start_ts), "end_utc": _format_ts(end_ts)},
        "top_apps": top_apps,
        "top_titles": top_titles,
        "key_events": key_events,
        "source": "realtime",
        "notes": [
            "Recent events only; use with pattern summaries for long-term context.",
            "Sensitive titles are redacted.",
        ],
    }

    payload = _compress_payload(payload, args.max_bytes)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"realtime_llm_input_saved={out_path}")


if __name__ == "__main__":
    main()
