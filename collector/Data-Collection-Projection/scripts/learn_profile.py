from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional
from urllib.parse import urlparse


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Learn personalization profile from recent events")
    parser.add_argument("--config", default="", help="optional config path")
    parser.add_argument("--db", default="", help="override db path")
    parser.add_argument("--days", type=int, default=14, help="lookback window in days")
    parser.add_argument("--output", default="configs/profile_learned.json", help="output json path")
    parser.add_argument("--min-events", type=int, default=100, help="minimum events required")
    parser.add_argument("--min-hours", type=int, default=3, help="minimum active hours for window")
    parser.add_argument("--max-tools", type=int, default=3)
    return parser.parse_args()


def _load_config(args: argparse.Namespace):
    if not args.config:
        return None
    try:
        import sys

        sys.path.insert(0, str(PROJECT_ROOT / "src"))
        from collector.config import load_config

        return load_config(Path(args.config))
    except Exception:
        return None


def _load_db_path(args: argparse.Namespace, config) -> Path:
    if args.db:
        return Path(args.db)
    if config is not None:
        try:
            return Path(config.db_path)
        except Exception:
            return Path("collector.db")
    return Path("collector.db")


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _safe_domain(value: str) -> str:
    if not value:
        return ""
    raw = str(value).strip()
    if not raw:
        return ""
    try:
        if "://" not in raw:
            raw = f"http://{raw}"
        parsed = urlparse(raw)
        domain = parsed.netloc or ""
        return domain.lower()
    except Exception:
        return ""


def _derive_working_hours(hour_counts: Counter[int], min_hours: int) -> str:
    if not hour_counts:
        return ""
    max_count = max(hour_counts.values())
    threshold = max(2, int(max_count * 0.2))
    active = sorted([hour for hour, cnt in hour_counts.items() if cnt >= threshold])
    if len(active) < min_hours:
        active = [hour for hour, _ in hour_counts.most_common(min_hours)]
    if not active:
        return ""
    start = min(active)
    end = (max(active) + 1) % 24
    return f"{start:02d}:00-{end:02d}:00"


def _tool_from_app(app_name: str) -> Optional[str]:
    name = (app_name or "").lower()
    patterns = {
        "notion": "notion",
        "slack": "slack",
        "kakaotalk": "slack",
        "teams": "slack",
        "outlook": "gmail",
        "gmail": "gmail",
        "thunderbird": "gmail",
    }
    for key, tool in patterns.items():
        if key in name:
            return tool
    return None


def _tool_from_domain(domain: str) -> Optional[str]:
    if not domain:
        return None
    mapping = {
        "notion.so": "notion",
        "slack.com": "slack",
        "teams.microsoft.com": "slack",
        "mail.google.com": "gmail",
        "outlook.office.com": "gmail",
        "outlook.live.com": "gmail",
    }
    for key, tool in mapping.items():
        if domain == key or domain.endswith(f".{key}"):
            return tool
    return None


def main() -> None:
    args = parse_args()
    config = _load_config(args)
    db_path = _load_db_path(args, config)

    tz_name = "local"
    if config is not None:
        try:
            tz_name = str(config.logging.timezone or "local")
        except Exception:
            tz_name = "local"

    cutoff = datetime.now(timezone.utc) - timedelta(days=max(1, int(args.days)))
    cutoff_iso = cutoff.isoformat().replace("+00:00", "Z")

    if not db_path.exists():
        output = {
            "generated_at": _now_utc(),
            "note": "db_not_found",
            "db_path": str(db_path),
        }
        Path(args.output).write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"profile_saved={args.output}")
        return

    conn = sqlite3.connect(str(db_path))
    cursor = conn.cursor()
    rows = cursor.execute(
        """
        SELECT ts, app, payload_json
        FROM events
        WHERE ts >= ?
        ORDER BY ts ASC
        """,
        (cutoff_iso,),
    ).fetchall()
    conn.close()

    if len(rows) < int(args.min_events):
        output = {
            "generated_at": _now_utc(),
            "window_days": args.days,
            "note": "insufficient_events",
            "event_count": len(rows),
        }
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        Path(args.output).write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"profile_saved={args.output}")
        return

    app_counts: Counter[str] = Counter()
    tool_counts: Counter[str] = Counter()
    domain_counts: Counter[str] = Counter()
    hour_counts: Counter[int] = Counter()

    for ts, app, payload_json in rows:
        app_name = str(app or "unknown")
        app_counts[app_name] += 1
        tool = _tool_from_app(app_name)
        if tool:
            tool_counts[tool] += 1

        payload = {}
        if payload_json:
            try:
                payload = json.loads(payload_json)
            except Exception:
                payload = {}
        domain = _safe_domain(payload.get("domain") or payload.get("url") or payload.get("page_url") or "")
        if domain:
            domain_counts[domain] += 1
            domain_tool = _tool_from_domain(domain)
            if domain_tool:
                tool_counts[domain_tool] += 1

        try:
            ts_dt = datetime.fromisoformat(str(ts).replace("Z", "+00:00"))
        except Exception:
            ts_dt = None
        if ts_dt is not None:
            if tz_name not in {"", "local", "system", "default"}:
                try:
                    from zoneinfo import ZoneInfo

                    ts_dt = ts_dt.astimezone(ZoneInfo(tz_name))
                except Exception:
                    ts_dt = ts_dt.astimezone()
            else:
                ts_dt = ts_dt.astimezone()
            hour_counts[ts_dt.hour] += 1

    preferred_tools = [tool for tool, _ in tool_counts.most_common(int(args.max_tools))]
    working_hours = _derive_working_hours(hour_counts, int(args.min_hours))

    output = {
        "generated_at": _now_utc(),
        "window_days": args.days,
        "tools": preferred_tools,
        "preferences": {
            "working_hours": working_hours,
            "timezone": tz_name,
        },
        "signals": {
            "top_apps": [{"app": app, "events": int(cnt)} for app, cnt in app_counts.most_common(6)],
            "top_domains": [
                {"domain": domain, "events": int(cnt)} for domain, cnt in domain_counts.most_common(6)
            ],
            "hour_counts": [{"hour": hour, "events": int(cnt)} for hour, cnt in hour_counts.most_common(8)],
        },
    }

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"profile_saved={args.output}")


if __name__ == "__main__":
    main()
