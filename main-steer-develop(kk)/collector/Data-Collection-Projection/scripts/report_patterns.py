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
    parser = argparse.ArgumentParser(description="Report hourly activity patterns")
    parser.add_argument("--config", default="configs/config.yaml", help="config path")
    parser.add_argument("--since-days", type=int, default=3, help="lookback window")
    parser.add_argument("--output", default="", help="optional markdown output path")
    parser.add_argument("--top-apps", type=int, default=5, help="top apps per hour")
    parser.add_argument("--top-titles", type=int, default=3, help="top titles overall")
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


def _fmt_dt(dt_value: datetime) -> str:
    return dt_value.strftime("%Y-%m-%d %H:%M:%S")


def _fmt_hhmm(seconds: int) -> str:
    if seconds <= 0:
        return "0m"
    minutes = seconds // 60
    hours, minutes = divmod(minutes, 60)
    if hours <= 0:
        return f"{minutes}m"
    return f"{hours}h {minutes}m"


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    tzinfo = _resolve_tz(getattr(config.logging, "timezone", "local"))

    cutoff = datetime.now(timezone.utc) - timedelta(days=max(1, args.since_days))
    conn = sqlite3.connect(str(config.db_path))
    cur = conn.cursor()
    rows = cur.execute(
        "SELECT ts, app, event_type, payload_json FROM events WHERE event_type = 'os.app_focus_block'"
    ).fetchall()

    hourly = defaultdict(Counter)
    hourly_by_day = defaultdict(lambda: defaultdict(Counter))
    total_by_app = Counter()
    titles = Counter()
    transitions = Counter()
    last_app = None
    durations = []

    for ts_raw, app, event_type, payload_json in rows:
        ts = parse_ts(ts_raw)
        if ts is None or ts < cutoff:
            continue
        if tzinfo:
            ts_local = ts.astimezone(tzinfo)
        else:
            ts_local = ts.astimezone()
        hour = ts_local.hour
        day_key = ts_local.strftime("%Y-%m-%d")
        try:
            payload = json.loads(payload_json or "{}")
        except Exception:
            payload = {}
        duration = payload.get("duration_sec") or 0
        try:
            duration = int(duration)
        except Exception:
            duration = 0
        if duration > 0:
            durations.append(duration)
        app_key = app or "UNKNOWN"
        hourly[hour][app_key] += duration
        hourly_by_day[day_key][hour][app_key] += duration
        total_by_app[app_key] += duration
        if last_app and last_app != app_key:
            transitions[(last_app, app_key)] += 1
        last_app = app_key
        title = payload.get("window_title")
        if isinstance(title, str) and title.strip():
            titles[title.strip()] += duration

    conn.close()

    output_lines = []
    output_lines.append("# Pattern Report (Hourly)\n")
    output_lines.append(f"- Lookback: last {args.since_days} days\n")
    output_lines.append(f"- DB: {config.db_path}\n")
    output_lines.append("")

    if total_by_app:
        top_app, top_sec = total_by_app.most_common(1)[0]
        top_title = titles.most_common(1)[0][0] if titles else ""
        summary_lines = []
        summary_lines.append(
            f"- 가장 많이 사용한 앱: {top_app} ({_fmt_hhmm(int(top_sec))})"
        )
        if top_title:
            summary_lines.append(f"- 가장 많이 등장한 타이틀: {top_title}")
        if hourly:
            hour_peak, apps = max(hourly.items(), key=lambda item: sum(item[1].values()))
            summary_lines.append(f"- 활동이 가장 집중된 시간대: {hour_peak:02d}시")
        if durations:
            summary_lines.append(
                f"- 집중 블록 평균 길이: {_fmt_hhmm(_avg(durations))}"
            )
        output_lines.append("## 0) 사용 패턴 요약\n")
        output_lines.extend(summary_lines)
        output_lines.append("")

    output_lines.append("## 0-1) 시간대 버킷 요약\n")
    bucket_usage = _build_time_buckets(hourly)
    for bucket, items in bucket_usage.items():
        if not items:
            continue
        top_app, top_sec = items[0]
        output_lines.append(f"- {bucket}: {top_app} {_fmt_hhmm(int(top_sec))}")
    output_lines.append("")

    output_lines.append("## 1) 시간대별 상위 앱(누적 사용시간)\n")
    for hour in range(24):
        if hour not in hourly:
            continue
        top = hourly[hour].most_common(args.top_apps)
        top_str = ", ".join([f"{app} {sec//60}m" for app, sec in top])
        output_lines.append(f"- {hour:02d}시 {top_str}")
    output_lines.append("")

    output_lines.append("## 2) 시간대별 대표 앱(일자별 최상위 다수결)\n")
    for hour in range(24):
        vote = Counter()
        for day_key, by_hour in hourly_by_day.items():
            if hour not in by_hour:
                continue
            top_app = by_hour[hour].most_common(1)[0][0]
            vote[top_app] += 1
        if not vote:
            continue
        winner, days = vote.most_common(1)[0]
        output_lines.append(f"- {hour:02d}시 {winner} (n={days} days)")
    output_lines.append("")

    output_lines.append("## 3) 전체 사용량 Top\n")
    for app, sec in total_by_app.most_common(10):
        output_lines.append(f"- {app}: {sec//60}m")
    output_lines.append("")

    output_lines.append("## 4-1) 앱 전환 Top\n")
    for (left, right), count in transitions.most_common(10):
        output_lines.append(f"- {left} → {right}: {count}회")
    output_lines.append("")

    output_lines.append("## 4) 상세 타이틀 Top\n")
    for title, sec in titles.most_common(args.top_titles):
        output_lines.append(f"- {title}: {sec//60}m")
    output_lines.append("")

    output_lines.append("## 5) 해석 가이드\n")
    output_lines.append("- 시간대별 대표 앱은 단순 다수결로 계산되며 패턴 참고용입니다.")
    output_lines.append("- title은 window_title 기반이라 앱 내부 상세 정보는 제한적일 수 있습니다.")
    output_lines.append("")

    report = "\n".join(output_lines)
    if args.output:
        Path(args.output).write_text(report, encoding="utf-8")
        print(f"report saved: {args.output}")
    else:
        print(report)


if __name__ == "__main__":
    main()


def _build_time_buckets(hourly: dict[int, Counter]) -> dict[str, list[tuple[str, int]]]:
    buckets = {
        "night(00-05)": range(0, 6),
        "morning(06-11)": range(6, 12),
        "afternoon(12-17)": range(12, 18),
        "evening(18-23)": range(18, 24),
    }
    output: dict[str, Counter] = {name: Counter() for name in buckets}
    for bucket_name, hours in buckets.items():
        for hour in hours:
            if hour not in hourly:
                continue
            for app, sec in hourly[hour].items():
                output[bucket_name][app] += sec
    return {name: counter.most_common(3) for name, counter in output.items()}


def _avg(values: list[int]) -> int:
    if not values:
        return 0
    return int(sum(values) / len(values))
