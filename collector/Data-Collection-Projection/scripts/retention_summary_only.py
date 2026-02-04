from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
from pathlib import Path

import sys

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config
from collector.store import SQLiteStore


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run retention on summary DB only")
    parser.add_argument("--config", default="configs/config_run4.yaml")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    summary_db = config.summary_db_path
    if not summary_db:
        print("summary_db_path not set")
        return

    store = SQLiteStore(summary_db, wal_mode=config.wal_mode, busy_timeout_ms=config.store.busy_timeout_ms)
    store.connect()
    store.migrate(Path(config.migrations_path))

    now = datetime.now(timezone.utc)
    cutoff_daily = _format_ts(now - timedelta(days=config.retention.daily_summaries_days))
    cutoff_pattern = _format_ts(now - timedelta(days=config.retention.pattern_summaries_days))
    cutoff_llm = _format_ts(now - timedelta(days=config.retention.llm_inputs_days))

    deleted_daily = store.delete_old_daily_summaries(cutoff_daily, batch_size=config.retention.batch_size)
    deleted_pattern = store.delete_old_pattern_summaries(cutoff_pattern, batch_size=config.retention.batch_size)
    deleted_llm = store.delete_old_llm_inputs(cutoff_llm, batch_size=config.retention.batch_size)

    store.checkpoint_wal()
    store.vacuum()
    store.close()

    print(
        f"summary_retention deleted_daily={deleted_daily} deleted_pattern={deleted_pattern} deleted_llm={deleted_llm}"
    )


def _format_ts(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    main()
