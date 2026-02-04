from __future__ import annotations

import argparse
import sqlite3
import sys
from pathlib import Path

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Show aggregated activity details (by app + title)"
    )
    parser.add_argument(
        "--config", default="configs/config.yaml", help="path to config file"
    )
    parser.add_argument("--app", default="", help="filter by app (case-insensitive)")
    parser.add_argument(
        "--limit", type=int, default=50, help="max rows to print"
    )
    parser.add_argument(
        "--order",
        choices=["duration", "blocks", "last_seen"],
        default="duration",
        help="sort order",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config = load_config(args.config)

    order_map = {
        "duration": "total_duration_sec DESC",
        "blocks": "blocks DESC",
        "last_seen": "last_seen_ts DESC",
    }
    order_sql = order_map[args.order]

    query = (
        "SELECT app, title_hint, total_duration_sec, blocks, last_seen_ts "
        "FROM activity_details"
    )
    params = []
    app_filter = args.app.strip().lower()
    if app_filter:
        query += " WHERE lower(app) = ?"
        params.append(app_filter)
    query += f" ORDER BY {order_sql} LIMIT ?"
    params.append(max(1, int(args.limit)))

    conn = sqlite3.connect(str(config.db_path))
    try:
        rows = conn.execute(query, params).fetchall()
    finally:
        conn.close()

    if not rows:
        print("no activity_details found")
        return

    for app, title, duration, blocks, last_seen in rows:
        title_display = title or "(no hint)"
        minutes = (duration or 0) / 60
        print(
            f"{app} | {title_display} | {minutes:.1f}m | blocks={blocks} | last={last_seen}"
        )


if __name__ == "__main__":
    main()
