from __future__ import annotations

import argparse
import json
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from statistics import mean


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate pattern quality")
    parser.add_argument(
        "--summaries-dir",
        default="logs",
        help="directory containing daily_summary_*.json",
    )
    parser.add_argument(
        "--pattern",
        default="",
        help="pattern_summary.json path (default: <summaries-dir>/pattern_summary.json)",
    )
    parser.add_argument(
        "--output",
        default="",
        help="optional output path for JSON report",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    summaries_dir = Path(args.summaries_dir)
    pattern_path = Path(args.pattern) if args.pattern else summaries_dir / "pattern_summary.json"

    pattern = _load_json(pattern_path)
    patterns = pattern.get("patterns") or []
    weekday_patterns = pattern.get("weekday_patterns") or {}
    sequence_patterns = pattern.get("sequence_patterns") or []

    daily_summaries = []
    for path in summaries_dir.glob("daily_summary_*.json"):
        daily = _load_json(path)
        if daily:
            daily_summaries.append(daily)

    hour_lookup = {p.get("hour"): p for p in patterns if p.get("hour")}
    weekday_lookup = {
        weekday: {p.get("hour"): p for p in items if p.get("hour")}
        for weekday, items in weekday_patterns.items()
        if isinstance(items, list)
    }

    total_hours = 0
    matched_hours = 0
    weekday_total = defaultdict(int)
    weekday_matched = defaultdict(int)

    for daily in daily_summaries:
        date_local = daily.get("date_local")
        weekday = ""
        if date_local:
            try:
                weekday = datetime.strptime(date_local, "%Y-%m-%d").strftime("%a")
            except Exception:
                weekday = ""
        hourly_usage = daily.get("hourly_usage") or {}
        for hour, items in hourly_usage.items():
            if not items:
                continue
            total_hours += 1
            top_app = items[0].get("app")
            pattern_item = hour_lookup.get(hour)
            if pattern_item and pattern_item.get("app") == top_app:
                matched_hours += 1
            if weekday and weekday in weekday_lookup:
                weekday_total[weekday] += 1
                weekday_item = weekday_lookup[weekday].get(hour)
                if weekday_item and weekday_item.get("app") == top_app:
                    weekday_matched[weekday] += 1

    confidences = [p.get("confidence") for p in patterns if p.get("confidence") is not None]
    avg_conf = round(mean(confidences), 3) if confidences else 0.0
    coverage = round(matched_hours / total_hours, 3) if total_hours else 0.0

    weekday_coverage = {}
    for weekday, total in weekday_total.items():
        matched = weekday_matched.get(weekday, 0)
        weekday_coverage[weekday] = round(matched / total, 3) if total else 0.0

    report = {
        "summary_count": len(daily_summaries),
        "pattern_hours": len(patterns),
        "pattern_coverage": coverage,
        "avg_confidence": avg_conf,
        "weekday_coverage": weekday_coverage,
        "sequence_patterns": sequence_patterns[:5],
    }

    output = json.dumps(report, ensure_ascii=False, indent=2)
    if args.output:
        Path(args.output).write_text(output, encoding="utf-8")
        print(f"pattern_quality_saved={args.output}")
    else:
        print(output)


def _load_json(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


if __name__ == "__main__":
    main()
