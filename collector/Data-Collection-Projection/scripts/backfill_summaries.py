from __future__ import annotations

import argparse
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict

import sys

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Backfill summaries into summary DB")
    parser.add_argument("--config", default="configs/config_run4.yaml")
    parser.add_argument("--summaries-dir", default="logs/run4")
    parser.add_argument("--daily-glob", default="daily_summary_*.json")
    parser.add_argument("--pattern", default="pattern_summary.json")
    parser.add_argument("--llm-input", default="llm_input.json")
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    summary_db_path = config.summary_db_path or config.db_path
    summaries_dir = Path(args.summaries_dir)

    conn = sqlite3.connect(str(summary_db_path))
    cur = conn.cursor()
    _ensure_summary_tables(cur)

    daily_count = 0
    for path in summaries_dir.glob(args.daily_glob):
        payload = _load_json(path)
        if not payload:
            continue
        date_local = payload.get("date_local") or path.stem.replace("daily_summary_", "")
        start_utc = payload.get("window", {}).get("start_utc") or ""
        end_utc = payload.get("window", {}).get("end_utc") or ""
        created_at = _now_utc()
        payload_json = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        if not args.dry_run:
            cur.execute(
                """
                INSERT INTO daily_summaries (date_local, start_utc, end_utc, payload_json, created_at)
                VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(date_local) DO UPDATE SET
                    payload_json = excluded.payload_json,
                    start_utc = excluded.start_utc,
                    end_utc = excluded.end_utc,
                    created_at = excluded.created_at
                """,
                (date_local, start_utc, end_utc, payload_json, created_at),
            )
        daily_count += 1

    pattern_path = summaries_dir / args.pattern
    pattern_payload = _load_json(pattern_path)
    if pattern_payload:
        created_at = pattern_payload.get("generated_at") or _now_utc()
        window_days = pattern_payload.get("window_days", 0)
        payload_json = json.dumps(pattern_payload, ensure_ascii=False, separators=(",", ":"))
        if not args.dry_run:
            cur.execute(
                """
                INSERT INTO pattern_summaries (created_at, window_days, payload_json)
                VALUES (?, ?, ?)
                """,
                (created_at, window_days, payload_json),
            )

    llm_path = summaries_dir / args.llm_input
    llm_payload = _load_json(llm_path)
    if llm_payload:
        created_at = llm_payload.get("generated_at") or _now_utc()
        payload_json = json.dumps(llm_payload, ensure_ascii=False, separators=(",", ":"))
        payload_size = len(payload_json.encode("utf-8"))
        if not args.dry_run:
            cur.execute(
                """
                INSERT INTO llm_inputs (created_at, payload_json, payload_size)
                VALUES (?, ?, ?)
                """,
                (created_at, payload_json, payload_size),
            )

    if not args.dry_run:
        conn.commit()
    conn.close()

    print(f"daily_backfilled={daily_count} pattern={bool(pattern_payload)} llm={bool(llm_payload)} dry_run={args.dry_run}")


def _ensure_summary_tables(cur: sqlite3.Cursor) -> None:
    migrations_path = PROJECT_ROOT / "migrations" / "007_summaries.sql"
    if migrations_path.exists():
        cur.executescript(migrations_path.read_text(encoding="utf-8"))


def _load_json(path: Path) -> Dict[str, Any]:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    main()
