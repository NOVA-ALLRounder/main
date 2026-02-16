from __future__ import annotations

import argparse
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
    parser = argparse.ArgumentParser(description="Summarize activity_details table")
    parser.add_argument("--config", default="configs/config.yaml", help="config path")
    parser.add_argument("--since-hours", type=int, default=24, help="lookback window")
    parser.add_argument("--top-apps", type=int, default=10, help="top apps to show")
    parser.add_argument("--top-titles", type=int, default=5, help="top titles per app")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    cutoff = datetime.now(timezone.utc) - timedelta(hours=max(1, args.since_hours))

    conn = sqlite3.connect(str(config.db_path))
    try:
        rows = conn.execute(
            "SELECT app, title_hint, total_duration_sec, blocks, last_seen_ts "
            "FROM activity_details"
        ).fetchall()
    finally:
        conn.close()

    filtered = []
    for app, title, duration, blocks, last_seen in rows:
        ts = parse_ts(last_seen)
        if ts is None or ts < cutoff:
            continue
        filtered.append((app, title, duration or 0, blocks or 0, last_seen))

    if not filtered:
        print("no recent activity_details found")
        return

    app_totals: dict[str, int] = {}
    by_app: dict[str, list[tuple[str, int, int, str]]] = {}
    for app, title, duration, blocks, last_seen in filtered:
        app_key = app or "UNKNOWN"
        app_totals[app_key] = app_totals.get(app_key, 0) + int(duration)
        by_app.setdefault(app_key, []).append(
            (title or "(no title)", int(duration), int(blocks), last_seen)
        )

    print("=== Activity Summary ===")
    for app, total in sorted(app_totals.items(), key=lambda x: x[1], reverse=True)[
        : max(1, args.top_apps)
    ]:
        minutes = total / 60
        print(f"{app}: {minutes:.1f}m")
        items = sorted(by_app.get(app, []), key=lambda x: x[1], reverse=True)[
            : max(1, args.top_titles)
        ]
        for title, duration, blocks, last_seen in items:
            print(
                f"  - {title} | {duration/60:.1f}m | blocks={blocks} | last={last_seen}"
            )


if __name__ == "__main__":
    main()
