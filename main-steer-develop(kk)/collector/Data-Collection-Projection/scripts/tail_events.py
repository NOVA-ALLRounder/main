from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Tail detailed events from the collector DB")
    parser.add_argument("--db", default="collector.db", help="db path")
    parser.add_argument("--apps", default="", help="comma-separated app filter, e.g. chrome.exe,code.exe")
    parser.add_argument("--since-minutes", type=int, default=5)
    parser.add_argument("--poll", type=float, default=1.0)
    parser.add_argument("--limit", type=int, default=200)
    parser.add_argument("--drop-idle", action="store_true")
    return parser.parse_args()


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _format_ts(dt: datetime) -> str:
    return dt.isoformat().replace("+00:00", "Z")


def main() -> None:
    args = parse_args()
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

    db_path = Path(args.db)
    if not db_path.is_absolute():
        db_path = Path(__file__).resolve().parents[1] / db_path

    app_filter = {a.strip().lower() for a in args.apps.split(",") if a.strip()}
    last_ts = _format_ts(_now_utc() - timedelta(minutes=max(1, args.since_minutes)))

    while True:
        conn = sqlite3.connect(str(db_path))
        cur = conn.cursor()
        rows = cur.execute(
            "SELECT ts, app, event_type, payload_json FROM events WHERE ts > ? ORDER BY ts ASC LIMIT ?",
            (last_ts, int(args.limit)),
        ).fetchall()
        conn.close()

        for ts, app, event_type, payload_json in rows:
            last_ts = ts or last_ts
            app_l = str(app or "").lower()
            if app_filter and app_l not in app_filter:
                continue
            event_type_str = str(event_type or "")
            if args.drop_idle and event_type_str.startswith("os.idle_"):
                continue
            try:
                payload = json.loads(payload_json) if payload_json else {}
            except Exception:
                payload = {}
            record = {
                "ts": ts,
                "app": app,
                "event_type": event_type,
                "payload": payload,
            }
            print(json.dumps(record, ensure_ascii=False))

        time.sleep(max(0.2, float(args.poll)))


if __name__ == "__main__":
    main()
