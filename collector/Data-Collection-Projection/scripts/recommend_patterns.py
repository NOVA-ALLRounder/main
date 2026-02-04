from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
import sys

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config
from collector.utils.time import parse_ts


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Recommend usage patterns")
    parser.add_argument("--config", default="configs/config.yaml", help="config path")
    parser.add_argument("--since-days", type=int, default=7, help="lookback window")
    parser.add_argument(
        "--min-days", type=int, default=2, help="min days to treat a pattern as stable"
    )
    parser.add_argument(
        "--min-minutes", type=int, default=10, help="min minutes per hour bucket"
    )
    parser.add_argument("--top-hours", type=int, default=12, help="max hours to emit")
    parser.add_argument(
        "--format", choices=["json", "md"], default="json", help="output format"
    )
    parser.add_argument("--output", default="", help="optional output path")
    return parser.parse_args()


def _resolve_tz(name: str):
    if not name:
        return None
    if str(name).lower() in {"local", "system", "default"}:
        return None
    try:
        from zoneinfo import ZoneInfo
    except Exception:
        return None
    try:
        return ZoneInfo(str(name))
    except Exception:
        return None


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    tzinfo = _resolve_tz(getattr(config.logging, "timezone", "local"))

    cutoff = datetime.now(timezone.utc) - timedelta(days=max(1, args.since_days))
    conn = sqlite3.connect(str(config.db_path))
    cur = conn.cursor()
    rows = cur.execute(
        "SELECT ts, app, payload_json FROM events WHERE event_type = 'os.app_focus_block'"
    ).fetchall()

    hourly_by_day = defaultdict(lambda: defaultdict(Counter))
    totals_by_hour = defaultdict(Counter)

    for ts_raw, app, payload_json in rows:
        ts = parse_ts(ts_raw)
        if ts is None or ts < cutoff:
            continue
        ts_local = ts.astimezone(tzinfo) if tzinfo else ts.astimezone()
        day_key = ts_local.strftime("%Y-%m-%d")
        hour = ts_local.hour
        try:
            payload = json.loads(payload_json or "{}")
        except Exception:
            payload = {}
        duration = payload.get("duration_sec") or 0
        try:
            duration = int(duration)
        except Exception:
            duration = 0
        app_key = app or "UNKNOWN"
        hourly_by_day[day_key][hour][app_key] += duration
        totals_by_hour[hour][app_key] += duration

    conn.close()

    recommendations = []
    for hour in range(24):
        votes = Counter()
        minutes_per_app = totals_by_hour.get(hour, Counter())
        if not minutes_per_app:
            continue
        for day_key, by_hour in hourly_by_day.items():
            if hour not in by_hour:
                continue
            top_app, top_sec = by_hour[hour].most_common(1)[0]
            votes[top_app] += 1
        if not votes:
            continue
        top_app, day_count = votes.most_common(1)[0]
        total_minutes = minutes_per_app.get(top_app, 0) // 60
        if day_count < args.min_days or total_minutes < args.min_minutes:
            continue
        recommendations.append(
            {
                "hour": f"{hour:02d}:00",
                "app": top_app,
                "days": day_count,
                "minutes": int(total_minutes),
            }
        )

    recommendations = sorted(
        recommendations, key=lambda item: (item["days"], item["minutes"]), reverse=True
    )
    if args.top_hours and args.top_hours > 0:
        recommendations = recommendations[: args.top_hours]

    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "window_days": int(args.since_days),
        "min_days": int(args.min_days),
        "min_minutes": int(args.min_minutes),
        "recommendations": recommendations,
    }

    if args.format == "md":
        lines = ["# Pattern Recommendations", ""]
        lines.append(f"- Lookback: {args.since_days} days")
        lines.append(f"- Min days: {args.min_days}")
        lines.append(f"- Min minutes/hour: {args.min_minutes}")
        lines.append("")
        if not recommendations:
            lines.append("No stable hourly patterns found.")
        else:
            for rec in recommendations:
                lines.append(
                    f"- {rec['hour']} -> {rec['app']} (days={rec['days']}, minutes={rec['minutes']})"
                )
        output = "\n".join(lines)
    else:
        output = json.dumps(payload, ensure_ascii=False, indent=2)

    if args.output:
        Path(args.output).write_text(output, encoding="utf-8")
        print(f"report saved: {args.output}")
    else:
        print(output)


if __name__ == "__main__":
    main()
