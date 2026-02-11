from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path
import sys

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config
from collector.utils.time import parse_ts


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build daily summary dataset")
    parser.add_argument("--config", default="configs/config.yaml", help="config path")
    parser.add_argument(
        "--date",
        default="",
        help="local date YYYY-MM-DD (default: today)",
    )
    parser.add_argument(
        "--output",
        default="",
        help="output path (default: logs/<run>/daily_summary_YYYY-MM-DD.json)",
    )
    parser.add_argument("--top-apps", type=int, default=10)
    parser.add_argument("--top-titles", type=int, default=10)
    parser.add_argument("--top-hourly", type=int, default=3)
    parser.add_argument("--store-db", action="store_true", help="store summary in DB")
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


def _parse_date(value: str) -> date:
    if not value:
        return datetime.now().date()
    return datetime.strptime(value, "%Y-%m-%d").date()


def _table_exists(conn: sqlite3.Connection, name: str) -> bool:
    row = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
        (name,),
    ).fetchone()
    return row is not None


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    tzinfo = _resolve_tz(getattr(config.logging, "timezone", "local"))

    target_date = _parse_date(args.date)
    start_local = datetime.combine(target_date, time.min)
    end_local = datetime.combine(target_date, time.max)
    if tzinfo:
        start_local = start_local.replace(tzinfo=tzinfo)
        end_local = end_local.replace(tzinfo=tzinfo)
    else:
        start_local = start_local.astimezone()
        end_local = end_local.astimezone()
    start_utc = start_local.astimezone(timezone.utc)
    end_utc = end_local.astimezone(timezone.utc)

    summary_db_path = config.summary_db_path or config.db_path
    source_db_path = config.db_path or summary_db_path
    read_conn = sqlite3.connect(str(source_db_path))
    read_cur = read_conn.cursor()
    write_conn = sqlite3.connect(str(summary_db_path))
    write_cur = write_conn.cursor()
    _ensure_summary_tables(write_cur)

    events = read_cur.execute(
        "SELECT ts, app, event_type, payload_json, priority FROM events WHERE ts >= ? AND ts <= ?",
        (start_utc.isoformat().replace("+00:00", "Z"), end_utc.isoformat().replace("+00:00", "Z")),
    ).fetchall()

    apps = Counter()
    hourly = defaultdict(Counter)
    bucket_usage = defaultdict(Counter)
    key_events = Counter()
    idle_start = 0
    idle_end = 0
    focus_blocks = 0
    focus_durations: list[int] = []
    transitions = Counter()
    last_app = None

    p0_set = {item.lower() for item in config.priority.p0_event_types}
    p1_set = {item.lower() for item in config.priority.p1_event_types}

    for ts_raw, app, event_type, payload_json, priority in events:
        ts = parse_ts(ts_raw)
        if ts is None:
            continue
        ts_local = ts.astimezone(tzinfo) if tzinfo else ts.astimezone()
        hour = ts_local.hour

        event_type_l = (event_type or "").lower()
        if event_type_l == "os.idle_start":
            idle_start += 1
        if event_type_l == "os.idle_end":
            idle_end += 1
        if event_type_l == "os.app_focus_block":
            focus_blocks += 1

        if event_type_l in p0_set or event_type_l in p1_set:
            key_events[event_type_l] += 1

        if event_type_l == "os.app_focus_block":
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
            apps[app_key] += duration
            hourly[hour][app_key] += duration
            bucket = _bucket_for_hour(hour)
            if bucket:
                bucket_usage[bucket][app_key] += duration
            focus_durations.append(duration)
            if last_app and last_app != app_key:
                transitions[(last_app, app_key)] += 1
            last_app = app_key

    summary = {
        "date_local": target_date.isoformat(),
        "window": {
            "start_local": start_local.strftime("%Y-%m-%d %H:%M:%S"),
            "end_local": end_local.strftime("%Y-%m-%d %H:%M:%S"),
            "start_utc": start_utc.isoformat().replace("+00:00", "Z"),
            "end_utc": end_utc.isoformat().replace("+00:00", "Z"),
        },
        "counts": {
            "events_total": len(events),
            "focus_blocks": focus_blocks,
            "idle_start": idle_start,
            "idle_end": idle_end,
        },
        "top_apps": [
            {"app": app, "minutes": int(seconds // 60), "seconds": int(seconds)}
            for app, seconds in apps.most_common(args.top_apps)
        ],
        "hourly_usage": {},
        "key_events": dict(key_events),
        "focus_block_stats": _summarize_durations(focus_durations),
        "app_switches": int(sum(transitions.values())),
        "top_transitions": [
            {"from": pair[0], "to": pair[1], "count": int(count)}
            for pair, count in transitions.most_common(10)
        ],
        "time_buckets": {},
    }

    for hour in range(24):
        if hour not in hourly:
            continue
        top = hourly[hour].most_common(args.top_hourly)
        summary["hourly_usage"][f"{hour:02d}"] = [
            {"app": app, "minutes": int(sec // 60), "seconds": int(sec)}
            for app, sec in top
        ]

    for bucket_name, counter in bucket_usage.items():
        top = counter.most_common(args.top_hourly)
        summary["time_buckets"][bucket_name] = [
            {"app": app, "minutes": int(sec // 60), "seconds": int(sec)}
            for app, sec in top
        ]

    if _table_exists(read_conn, "activity_details"):
        rows = read_cur.execute(
            "SELECT app, title_hint, total_duration_sec, last_seen_ts FROM activity_details WHERE last_seen_ts >= ? AND last_seen_ts <= ?",
            (summary["window"]["start_utc"], summary["window"]["end_utc"]),
        ).fetchall()
        titles = Counter()
        for app, title_hint, total_duration_sec, last_seen_ts in rows:
            if not title_hint:
                continue
            try:
                total_duration_sec = int(total_duration_sec or 0)
            except Exception:
                total_duration_sec = 0
            titles[(app or "UNKNOWN", title_hint)] += total_duration_sec
        summary["top_titles"] = [
            {
                "app": app,
                "title_hint": title,
                "minutes": int(seconds // 60),
                "seconds": int(seconds),
            }
            for (app, title), seconds in titles.most_common(args.top_titles)
        ]
    else:
        summary["top_titles"] = []

    # ── actions 테이블에서 구체적 행동 집계 ──
    if _table_exists(read_conn, "actions"):
        action_rows = read_cur.execute(
            "SELECT action_type, app, window_title, control_name, "
            "final_value, started_at, duration_ms, metadata_json "
            "FROM actions WHERE started_at >= ? AND started_at <= ? "
            "ORDER BY started_at ASC",
            (summary["window"]["start_utc"], summary["window"]["end_utc"]),
        ).fetchall()

        summary["action_summary"] = _build_action_summary(action_rows)
    else:
        summary["action_summary"] = {
            "total_actions": 0, "by_type": {}, "app_action_profiles": [],
            "top_action_sequences": [],
        }

    if args.store_db:
        payload_json = json.dumps(summary, ensure_ascii=False, separators=(",", ":"))
        created_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        write_cur.execute(
            """
            INSERT INTO daily_summaries (date_local, start_utc, end_utc, payload_json, created_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(date_local) DO UPDATE SET
                payload_json = excluded.payload_json,
                start_utc = excluded.start_utc,
                end_utc = excluded.end_utc,
                created_at = excluded.created_at
            """,
            (
                summary["date_local"],
                summary["window"]["start_utc"],
                summary["window"]["end_utc"],
                payload_json,
                created_at,
            ),
        )
        write_conn.commit()

    read_conn.close()
    write_conn.close()

    if args.output:
        out_path = Path(args.output)
    else:
        log_dir = Path(config.logging.dir)
        out_path = log_dir / f"daily_summary_{target_date.isoformat()}.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"daily_summary_saved={out_path}")


def _ensure_summary_tables(cur: sqlite3.Cursor) -> None:
    migrations_path = Path(__file__).resolve().parents[1] / "migrations" / "007_summaries.sql"
    if migrations_path.exists():
        cur.executescript(migrations_path.read_text(encoding="utf-8"))



def _build_action_summary(action_rows: list) -> dict:
    """actions 테이블 rows를 분석하여 action_summary를 생성."""
    import re
    _EMAIL_RE = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
    _LONG_DIGITS_RE = re.compile(r"\b\d{8,}\b")

    def _mask_value(val: str) -> str:
        """민감한 값(이메일, 긴 숫자)을 마스킹."""
        if not val:
            return val
        val = _EMAIL_RE.sub("[이메일]", val)
        val = _LONG_DIGITS_RE.sub("[숫자]", val)
        if len(val) > 60:
            val = val[:57] + "..."
        return val

    total = len(action_rows)
    by_type: Counter = Counter()
    # 앱별 action 집계
    app_actions: dict = defaultdict(list)

    for action_type, app, window_title, control_name, final_value, started_at, duration_ms, metadata_json in action_rows:
        action_type = action_type or "unknown"
        app_key = app or "UNKNOWN"
        by_type[action_type] += 1

        step = {"type": action_type}
        if control_name:
            step["control"] = control_name
        if action_type == "input" and final_value:
            step["value_hint"] = _mask_value(final_value)
        elif action_type == "shortcut" and final_value:
            step["value"] = final_value
        elif action_type == "navigate" and window_title:
            step["target"] = _mask_value(window_title)

        app_actions[app_key].append(step)

    # ── 앱별 action 프로필 (자주 하는 행동 Top N) ──
    app_action_profiles = []
    for app_key, steps in app_actions.items():
        action_counter: Counter = Counter()
        for s in steps:
            # type + control/value 조합으로 고유 행동 식별
            key_parts = [s["type"]]
            if "control" in s:
                key_parts.append(s["control"])
            if "value" in s:
                key_parts.append(s["value"])
            action_counter[tuple(key_parts)] += 1

        top_actions = []
        for parts, count in action_counter.most_common(5):
            action_entry = {"type": parts[0], "count": count}
            if len(parts) > 1:
                if parts[0] == "shortcut":
                    action_entry["value"] = parts[1]
                else:
                    action_entry["control"] = parts[1]
            if len(parts) > 2:
                action_entry["value"] = parts[2]
            top_actions.append(action_entry)

        app_action_profiles.append({
            "app": app_key,
            "action_count": len(steps),
            "top_actions": top_actions,
        })

    # action_count 기준 내림차순 정렬, 상위 10개
    app_action_profiles.sort(key=lambda x: x["action_count"], reverse=True)
    app_action_profiles = app_action_profiles[:10]

    # ── 반복되는 action 시퀀스 감지 ──
    # 연속된 같은 앱의 action을 하나의 시퀀스로 그룹화
    sequences: list = []
    current_app = None
    current_steps: list = []

    for action_type, app, window_title, control_name, final_value, started_at, duration_ms, metadata_json in action_rows:
        app_key = app or "UNKNOWN"
        step = {"type": action_type or "unknown"}
        if control_name:
            step["control"] = control_name
        if action_type == "shortcut" and final_value:
            step["value"] = final_value
        elif action_type == "navigate" and window_title:
            step["target"] = _mask_value(window_title)

        if app_key == current_app:
            current_steps.append(step)
        else:
            if current_app and len(current_steps) >= 2:
                sequences.append({"app": current_app, "steps": current_steps})
            current_app = app_key
            current_steps = [step]

    if current_app and len(current_steps) >= 2:
        sequences.append({"app": current_app, "steps": current_steps})

    # 시퀀스 패턴 중복 감지 (step type+control 기준 시그니처 비교)
    sig_counter: Counter = Counter()
    sig_to_seq: dict = {}
    for seq in sequences:
        sig_parts = []
        for s in seq["steps"][:8]:  # 최대 8단계까지만
            sig = s["type"]
            if "control" in s:
                sig += ":" + s["control"]
            sig_parts.append(sig)
        signature = seq["app"] + "|" + "->".join(sig_parts)
        sig_counter[signature] += 1
        if signature not in sig_to_seq:
            sig_to_seq[signature] = seq

    top_sequences = []
    for sig, count in sig_counter.most_common(5):
        seq = sig_to_seq[sig]
        top_sequences.append({
            "app": seq["app"],
            "steps": seq["steps"][:8],
            "count": count,
        })

    return {
        "total_actions": total,
        "by_type": dict(by_type),
        "app_action_profiles": app_action_profiles,
        "top_action_sequences": top_sequences,
    }


def _bucket_for_hour(hour: int) -> str:
    if 0 <= hour < 6:
        return "night"
    if 6 <= hour < 12:
        return "morning"
    if 12 <= hour < 18:
        return "afternoon"
    if 18 <= hour < 24:
        return "evening"
    return "unknown"


def _summarize_durations(durations: list[int]) -> dict:
    if not durations:
        return {"count": 0, "avg_sec": 0, "median_sec": 0, "p90_sec": 0}
    values = sorted(max(0, int(value)) for value in durations)
    count = len(values)
    avg_sec = int(sum(values) / count)
    median_sec = values[count // 2]
    p90_index = min(count - 1, int(count * 0.9))
    p90_sec = values[p90_index]
    return {
        "count": count,
        "avg_sec": avg_sec,
        "median_sec": median_sec,
        "p90_sec": p90_sec,
    }


if __name__ == "__main__":
    main()
