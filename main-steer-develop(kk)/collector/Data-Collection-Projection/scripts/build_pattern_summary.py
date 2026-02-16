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
    parser = argparse.ArgumentParser(description="Build pattern summary dataset")
    parser.add_argument(
        "--summaries-dir",
        default="logs",
        help="directory containing daily_summary_*.json",
    )
    parser.add_argument(
        "--config",
        default="",
        help="optional config to read DB for sequence patterns",
    )
    parser.add_argument("--since-days", type=int, default=7)
    parser.add_argument("--ngram-min", type=int, default=2)
    parser.add_argument("--ngram-max", type=int, default=3)
    parser.add_argument("--top-sequences", type=int, default=10)
    parser.add_argument("--top-hours", type=int, default=12)
    parser.add_argument(
        "--include-apps",
        default="",
        help="comma-separated app allowlist for patterns",
    )
    parser.add_argument(
        "--hours",
        default="",
        help="hour filter, e.g. 9-18 or 9,10,11",
    )
    parser.add_argument("--output", default="", help="output path")
    parser.add_argument("--store-db", action="store_true", help="store summary in DB")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    summaries_dir = Path(args.summaries_dir)
    if summaries_dir.is_file():
        summaries_dir = summaries_dir.parent

    cutoff = datetime.now(timezone.utc) - timedelta(days=max(1, args.since_days))
    summaries = []
    for path in summaries_dir.glob("daily_summary_*.json"):
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            continue
        start_utc = raw.get("window", {}).get("start_utc")
        if not start_utc:
            continue
        try:
            start_dt = datetime.fromisoformat(start_utc.replace("Z", "+00:00"))
        except Exception:
            continue
        if start_dt < cutoff:
            continue
        summaries.append(raw)

    hourly_votes = defaultdict(Counter)
    hourly_minutes = defaultdict(Counter)
    app_totals = Counter()
    focus_stats = []
    weekday_votes = defaultdict(lambda: defaultdict(Counter))
    weekday_minutes = defaultdict(lambda: defaultdict(Counter))
    include_apps = _parse_apps(args.include_apps)
    include_hours = _parse_hours(args.hours)

    for summary in summaries:
        weekday_name = ""
        if summary.get("date_local"):
            try:
                weekday_name = datetime.strptime(summary["date_local"], "%Y-%m-%d").strftime("%a")
            except Exception:
                weekday_name = ""
        for hour_str, items in (summary.get("hourly_usage") or {}).items():
            if not items:
                continue
            if include_hours and int(hour_str) not in include_hours:
                continue
            if include_apps:
                items = [item for item in items if item.get("app") in include_apps]
                if not items:
                    continue
            top_app = items[0]["app"]
            hourly_votes[hour_str][top_app] += 1
            for item in items:
                hourly_minutes[hour_str][item["app"]] += int(item.get("seconds", 0))
                if weekday_name:
                    weekday_minutes[weekday_name][hour_str][item["app"]] += int(item.get("seconds", 0))
            if weekday_name:
                weekday_votes[weekday_name][hour_str][top_app] += 1
        for item in summary.get("top_apps", []):
            if include_apps and item.get("app") not in include_apps:
                continue
            app_totals[item["app"]] += int(item.get("seconds", 0))
        stats = summary.get("focus_block_stats") or {}
        if stats.get("count"):
            focus_stats.append(stats)

    patterns = []
    for hour_str in sorted(hourly_votes.keys()):
        winner, days = hourly_votes[hour_str].most_common(1)[0]
        minutes = hourly_minutes[hour_str][winner] // 60
        confidence = _confidence(days, len(summaries), minutes)
        patterns.append(
            {
                "hour": hour_str,
                "app": winner,
                "days": days,
                "minutes": int(minutes),
                "confidence": confidence,
            }
        )

    patterns = sorted(
        patterns, key=lambda item: (item["days"], item["minutes"]), reverse=True
    )
    if args.top_hours and args.top_hours > 0:
        patterns = patterns[: args.top_hours]

    weekday_patterns = {}
    for weekday, hours in weekday_votes.items():
        weekday_patterns[weekday] = []
        for hour_str in sorted(hours.keys()):
            winner, days = hours[hour_str].most_common(1)[0]
            minutes = weekday_minutes[weekday][hour_str][winner] // 60
            weekday_patterns[weekday].append(
                {
                    "hour": hour_str,
                    "app": winner,
                    "days": days,
                    "minutes": int(minutes),
                    "confidence": _confidence(days, len(summaries), minutes),
                }
            )

    sequence_patterns = []
    transition_patterns = []
    if args.config:
        sequence_patterns, transition_patterns = _build_sequences(
            args, include_apps, include_hours
        )

    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "window_days": int(args.since_days),
        "patterns": patterns,
        "weekday_patterns": weekday_patterns,
        "sequence_patterns": sequence_patterns,
        "transition_patterns": transition_patterns,
        "time_bucket_patterns": _build_time_bucket_patterns(summaries),
        "focus_block_stats": _aggregate_focus_stats(focus_stats),
        "top_apps": [
            {"app": app, "minutes": int(sec // 60), "seconds": int(sec)}
            for app, sec in app_totals.most_common(10)
        ],
        "summary_count": len(summaries),
    }

    if args.output:
        out_path = Path(args.output)
    else:
        out_path = summaries_dir / "pattern_summary.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"pattern_summary_saved={out_path}")

    if args.store_db and args.config:
        config = load_config(args.config)
        summary_db_path = config.summary_db_path or config.db_path
        conn = sqlite3.connect(str(summary_db_path))
        cur = conn.cursor()
        _ensure_summary_tables(cur)
        created_at = payload.get("generated_at")
        window_days = payload.get("window_days", 0)
        payload_json = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        cur.execute(
            """
            INSERT INTO pattern_summaries (created_at, window_days, payload_json)
            VALUES (?, ?, ?)
            """,
            (created_at, window_days, payload_json),
        )
        conn.commit()
        conn.close()


def _ensure_summary_tables(cur: sqlite3.Cursor) -> None:
    migrations_path = Path(__file__).resolve().parents[1] / "migrations" / "007_summaries.sql"
    if migrations_path.exists():
        cur.executescript(migrations_path.read_text(encoding="utf-8"))


def _confidence(days: int, total_days: int, minutes: int) -> float:
    if total_days <= 0:
        return 0.0
    day_ratio = min(1.0, days / max(1, total_days))
    minutes_ratio = min(1.0, minutes / 30.0)
    return round(day_ratio * 0.7 + minutes_ratio * 0.3, 3)


def _build_sequences(
    args: argparse.Namespace,
    include_apps: set[str],
    include_hours: set[int],
) -> tuple[list[dict], list[dict]]:
    config = load_config(args.config)
    cutoff = datetime.now(timezone.utc) - timedelta(days=max(1, args.since_days))

    tzinfo = None
    if getattr(config.logging, "timezone", ""):
        try:
            from zoneinfo import ZoneInfo
            if str(config.logging.timezone).lower() not in {"local", "system", "default"}:
                tzinfo = ZoneInfo(str(config.logging.timezone))
        except Exception:
            tzinfo = None

    sequences: list[list[str]] = []
    current_day = None
    day_apps: list[str] = []

    if config.summary_db_path:
        # Use daily summaries (already aggregated) to build coarse sequences.
        summary_conn = sqlite3.connect(str(config.summary_db_path))
        summary_cur = summary_conn.cursor()
        rows = summary_cur.execute(
            "SELECT payload_json FROM daily_summaries ORDER BY date_local ASC"
        ).fetchall()
        for (payload_json,) in rows:
            try:
                payload = json.loads(payload_json)
            except Exception:
                continue
            hourly = payload.get("hourly_usage") or {}
            seq = []
            for hour in sorted(hourly.keys()):
                if include_hours and int(hour) not in include_hours:
                    continue
                items = hourly.get(hour) or []
                if not items:
                    continue
                app_key = items[0].get("app") or "UNKNOWN"
                if include_apps and app_key not in include_apps:
                    continue
                seq.append(app_key)
            if seq:
                sequences.append(seq)
        summary_conn.close()
    else:
        conn = sqlite3.connect(str(config.db_path))
        cur = conn.cursor()
        rows = cur.execute(
            "SELECT ts, app FROM events WHERE event_type = 'os.app_focus_block' ORDER BY ts ASC"
        ).fetchall()

        for ts_raw, app in rows:
            ts = parse_ts(ts_raw)
            if ts is None or ts < cutoff:
                continue
            if include_hours:
                ts_local = ts.astimezone(tzinfo) if tzinfo else ts.astimezone()
                if ts_local.hour not in include_hours:
                    continue
            day_key = ts.date().isoformat()
            app_key = app or "UNKNOWN"
            if include_apps and app_key not in include_apps:
                continue
            if current_day is None:
                current_day = day_key
            if day_key != current_day:
                if day_apps:
                    sequences.append(day_apps)
                day_apps = []
                current_day = day_key
            day_apps.append(app_key)

        if day_apps:
            sequences.append(day_apps)

        conn.close()

    ngram_counts = Counter()
    total = 0
    for day_seq in sequences:
        for n in range(args.ngram_min, args.ngram_max + 1):
            for i in range(0, max(0, len(day_seq) - n + 1)):
                ngram = tuple(day_seq[i : i + n])
                ngram_counts[ngram] += 1
                total += 1

    results = []
    for ngram, count in ngram_counts.most_common(args.top_sequences):
        confidence = round(count / max(1, total), 3)
        results.append(
            {
                "sequence": list(ngram),
                "support": count,
                "confidence": confidence,
            }
        )
    transition_patterns = _build_transition_patterns(sequences, args.top_sequences)
    return results, transition_patterns


def _build_transition_patterns(
    sequences: list[list[str]], top_k: int
) -> list[dict]:
    transitions = Counter()
    for seq in sequences:
        for i in range(len(seq) - 1):
            left = seq[i]
            right = seq[i + 1]
            if not left or not right or left == right:
                continue
            transitions[(left, right)] += 1
    results = []
    for (left, right), count in transitions.most_common(top_k):
        results.append({"from": left, "to": right, "support": int(count)})
    return results


def _build_time_bucket_patterns(summaries: list[dict]) -> dict:
    bucket_minutes = defaultdict(Counter)
    bucket_days = defaultdict(Counter)
    for summary in summaries:
        buckets = summary.get("time_buckets") or {}
        for bucket_name, items in buckets.items():
            if not items:
                continue
            top_app = items[0].get("app")
            if top_app:
                bucket_days[bucket_name][top_app] += 1
            for item in items:
                app = item.get("app")
                if not app:
                    continue
                seconds = int(item.get("seconds", 0))
                bucket_minutes[bucket_name][app] += seconds

    output = {}
    for bucket_name, counts in bucket_days.items():
        winner, days = counts.most_common(1)[0]
        minutes = bucket_minutes[bucket_name][winner] // 60
        output[bucket_name] = {
            "app": winner,
            "days": int(days),
            "minutes": int(minutes),
        }
    return output


def _aggregate_focus_stats(items: list[dict]) -> dict:
    if not items:
        return {"avg_sec": 0, "median_sec": 0, "p90_sec": 0}
    avg_vals = [int(item.get("avg_sec", 0)) for item in items]
    med_vals = [int(item.get("median_sec", 0)) for item in items]
    p90_vals = [int(item.get("p90_sec", 0)) for item in items]
    def _median(values: list[int]) -> int:
        if not values:
            return 0
        values = sorted(values)
        return values[len(values) // 2]
    return {
        "avg_sec": int(sum(avg_vals) / max(1, len(avg_vals))),
        "median_sec": _median(med_vals),
        "p90_sec": _median(p90_vals),
    }


def _parse_apps(value: str) -> set[str]:
    if not value:
        return set()
    return {item.strip() for item in value.split(",") if item.strip()}


def _parse_hours(value: str) -> set[int]:
    if not value:
        return set()
    value = value.strip()
    hours = set()
    if "-" in value:
        start_s, end_s = value.split("-", 1)
        try:
            start = int(start_s)
            end = int(end_s)
            for hour in range(start, end + 1):
                hours.add(hour)
        except ValueError:
            return set()
    else:
        for part in value.split(","):
            try:
                hours.add(int(part.strip()))
            except ValueError:
                continue
    return hours


if __name__ == "__main__":
    main()
