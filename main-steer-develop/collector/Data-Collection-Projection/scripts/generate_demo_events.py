from __future__ import annotations

import argparse
import json
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate demo event JSONL for workflow testing")
    parser.add_argument(
        "--output",
        default="logs/demo_events_sequence.jsonl",
        help="output jsonl path",
    )
    parser.add_argument(
        "--apps",
        default="CHROME.EXE,NOTION.EXE,KAKAOTALK.EXE",
        help="comma-separated app sequence",
    )
    parser.add_argument("--cycles", type=int, default=3, help="sequence repeats")
    parser.add_argument("--start-minutes-ago", type=int, default=15)
    parser.add_argument("--include-key-events", action="store_true")
    return parser.parse_args()


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _event(
    ts: datetime,
    app: str,
    event_type: str,
    resource_id: str,
    *,
    duration_sec: int = 60,
) -> dict:
    return {
        "schema_version": "1.0",
        "event_id": f"demo-{app}-{event_type}-{resource_id}-{int(ts.timestamp())}",
        "ts": ts.isoformat().replace("+00:00", "Z"),
        "source": "os",
        "app": app,
        "event_type": event_type,
        "priority": "P2",
        "resource": {"type": "window", "id": resource_id},
        "payload": {
            "duration_sec": duration_sec,
            "window_title": f"{app} demo window",
            "action": "focus",
        },
    }


def main() -> None:
    args = parse_args()
    apps = [item.strip() for item in args.apps.split(",") if item.strip()]
    if not apps:
        raise SystemExit("no apps provided")

    start = _now_utc() - timedelta(minutes=max(1, int(args.start_minutes_ago)))
    events = []
    cursor = start
    step = timedelta(seconds=20)

    for cycle in range(max(1, int(args.cycles))):
        for idx, app in enumerate(apps):
            resource_id = f"{app.lower()}-{cycle}-{idx}"
            events.append(_event(cursor, app, "os.app_focus_block", resource_id))
            cursor += step
            if args.include_key_events and app.upper() == "CHROME.EXE":
                events.append(_event(cursor, app, "browser.tab_active", resource_id, duration_sec=5))
                cursor += step

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8") as fh:
        for event in events:
            fh.write(json.dumps(event, ensure_ascii=False) + "\n")
    print(f"demo_events_saved={out_path}")


if __name__ == "__main__":
    main()
