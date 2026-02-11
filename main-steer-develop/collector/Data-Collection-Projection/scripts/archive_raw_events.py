from __future__ import annotations

import argparse
import gzip
import json
import sqlite3
from datetime import datetime, timedelta, timezone
from pathlib import Path
import sys

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config
from collector.utils.time import parse_ts


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Archive raw events to jsonl.gz")
    parser.add_argument("--config", default="configs/config_run4.yaml")
    parser.add_argument("--date", default="", help="YYYY-MM-DD (local)")
    parser.add_argument("--days", type=int, default=1, help="number of days to archive")
    parser.add_argument("--output-dir", default="archive/raw")
    parser.add_argument("--delete-after", action="store_true", help="delete after archive")
    return parser.parse_args()


def _resolve_tz(name: str):
    if not name or str(name).lower() in {"local", "system", "default"}:
        return None
    try:
        from zoneinfo import ZoneInfo
    except Exception:
        return None
    try:
        return ZoneInfo(str(name))
    except Exception:
        return None


def _parse_date(value: str, tzinfo) -> datetime:
    if not value:
        now = datetime.now(tzinfo or timezone.utc)
        return datetime(now.year, now.month, now.day, tzinfo=tzinfo or timezone.utc)
    dt = datetime.strptime(value, "%Y-%m-%d")
    if tzinfo:
        return dt.replace(tzinfo=tzinfo)
    return dt.replace(tzinfo=timezone.utc)


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    tzinfo = _resolve_tz(getattr(config.logging, "timezone", "local"))
    start_local = _parse_date(args.date, tzinfo)
    days = max(1, int(args.days))

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(str(config.db_path))
    cur = conn.cursor()

    total_archived = 0
    for day in range(days):
        day_start = start_local + timedelta(days=day)
        day_end = day_start.replace(hour=23, minute=59, second=59, microsecond=999999)
        start_utc = day_start.astimezone(timezone.utc)
        end_utc = day_end.astimezone(timezone.utc)

        rows = cur.execute(
            """
            SELECT schema_version, event_id, ts, source, app, event_type, priority,
                   resource_type, resource_id, payload_json, privacy_json, pid, window_id, raw_json
            FROM events
            WHERE ts >= ? AND ts <= ?
            ORDER BY ts ASC
            """,
            (start_utc.isoformat().replace("+00:00", "Z"), end_utc.isoformat().replace("+00:00", "Z")),
        ).fetchall()

        if not rows:
            continue

        date_str = day_start.strftime("%Y-%m-%d")
        out_path = output_dir / f"raw_{date_str}.jsonl.gz"
        with gzip.open(out_path, "wt", encoding="utf-8") as f:
            for row in rows:
                (
                    schema_version,
                    event_id,
                    ts,
                    source,
                    app,
                    event_type,
                    priority,
                    resource_type,
                    resource_id,
                    payload_json,
                    privacy_json,
                    pid,
                    window_id,
                    raw_json,
                ) = row
                try:
                    payload = json.loads(payload_json or "{}")
                except Exception:
                    payload = {}
                try:
                    privacy = json.loads(privacy_json or "{}")
                except Exception:
                    privacy = {}
                try:
                    raw = json.loads(raw_json or "{}")
                except Exception:
                    raw = {}
                event = {
                    "schema_version": schema_version,
                    "event_id": event_id,
                    "ts": ts,
                    "source": source,
                    "app": app,
                    "event_type": event_type,
                    "priority": priority,
                    "resource": {"type": resource_type, "id": resource_id},
                    "payload": payload,
                    "privacy": privacy,
                    "pid": pid,
                    "window_id": window_id,
                    "raw": raw,
                }
                f.write(json.dumps(event, ensure_ascii=False) + "\n")

        total_archived += len(rows)

        if args.delete_after:
            cur.execute(
                "DELETE FROM events WHERE ts >= ? AND ts <= ?",
                (start_utc.isoformat().replace("+00:00", "Z"), end_utc.isoformat().replace("+00:00", "Z")),
            )
            conn.commit()

        print(f"archived {date_str} rows={len(rows)} -> {out_path}")

    conn.close()
    print(f"archive_done total_rows={total_archived}")


if __name__ == "__main__":
    main()
