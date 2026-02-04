from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path

from cryptography.fernet import Fernet


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check browser content capture ratio from encrypted raw_json"
    )
    parser.add_argument("--db", default="collector_run5.db", help="SQLite DB path")
    parser.add_argument(
        "--key-path", default="secrets/collector_key.txt", help="encryption key path"
    )
    parser.add_argument("--limit", type=int, default=200, help="rows to inspect")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    db_path = Path(args.db)
    if not db_path.exists():
        raise SystemExit(f"db not found: {db_path}")
    key_path = Path(args.key_path)
    if not key_path.exists():
        raise SystemExit(f"key not found: {key_path}")

    key = key_path.read_text(encoding="utf-8").strip().encode()
    fernet = Fernet(key)

    conn = sqlite3.connect(db_path)
    rows = conn.execute(
        "select raw_json from events where event_type='browser.tab_active' "
        "order by ts desc limit ?",
        (args.limit,),
    ).fetchall()
    conn.close()

    total = 0
    with_summary = 0
    summary_empty = 0
    with_content = 0
    for (raw_json,) in rows:
        try:
            obj = json.loads(raw_json)
        except Exception:
            continue
        if "__enc__" not in obj:
            continue
        try:
            decrypted = fernet.decrypt(obj["__enc__"].encode()).decode("utf-8")
            payload = json.loads(decrypted).get("payload", {})
        except Exception:
            continue
        total += 1
        summary = payload.get("content_summary")
        content = payload.get("content")
        if summary is not None:
            with_summary += 1
            if not str(summary).strip():
                summary_empty += 1
        if content:
            with_content += 1

    if total == 0:
        print("no encrypted browser events inspected")
        return
    print(
        f"checked={total} summary={with_summary} "
        f"summary_empty={summary_empty} content={with_content}"
    )
    print(
        f"summary_rate={with_summary/total:.2%} "
        f"content_rate={with_content/total:.2%}"
    )


if __name__ == "__main__":
    main()
