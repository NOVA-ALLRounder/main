from __future__ import annotations

import argparse
import json
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate mock focus-block events")
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--start-date", default="", help="YYYY-MM-DD (local)")
    parser.add_argument("--output", default="tests/fixtures/mock_events_pattern.jsonl")
    parser.add_argument("--tz-offset", default="+09:00", help="timezone offset like +09:00")
    return parser.parse_args()


def _parse_start_date(value: str) -> datetime:
    if value:
        dt = datetime.strptime(value, "%Y-%m-%d")
        return dt.replace(tzinfo=timezone.utc)
    now = datetime.now(timezone.utc)
    return datetime(now.year, now.month, now.day, tzinfo=timezone.utc)


def _tz_from_offset(offset: str) -> timezone:
    try:
        sign = 1 if offset.startswith("+") else -1
        parts = offset.replace("+", "").replace("-", "").split(":")
        hours = int(parts[0])
        minutes = int(parts[1]) if len(parts) > 1 else 0
        return timezone(sign * timedelta(hours=hours, minutes=minutes))
    except Exception:
        return timezone.utc


def main() -> None:
    args = parse_args()
    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    tzinfo = _tz_from_offset(args.tz_offset)
    start_date = _parse_start_date(args.start_date).astimezone(tzinfo)

    pattern = [
        (9, 30, "NOTION.EXE", "Daily Planning"),
        (10, 60, "CHROME.EXE", "Research"),
        (13, 45, "CODE.EXE", "Implementation"),
        (15, 30, "NOTION.EXE", "Notes"),
        (16, 40, "CHROME.EXE", "Review"),
    ]

    lines = []
    for day in range(max(1, args.days)):
        base = start_date + timedelta(days=day)
        for hour, minutes, app, title in pattern:
            ts = base.replace(hour=hour, minute=0, second=0, microsecond=0)
            payload = {
                "duration_sec": minutes * 60,
                "window_title": f"{title} - {app}",
            }
            event = {
                "schema_version": "1.0",
                "ts": ts.astimezone(timezone.utc).isoformat().replace("+00:00", "Z"),
                "source": "os",
                "app": app,
                "event_type": "os.app_focus_block",
                "resource": {"type": "window", "id": f"mock-{app}-{day}-{hour}"},
                "payload": payload,
            }
            lines.append(json.dumps(event, ensure_ascii=False))

    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"mock_events_saved={out_path} count={len(lines)}")


if __name__ == "__main__":
    main()
