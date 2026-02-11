from __future__ import annotations

import argparse
import json
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import List


@dataclass
class EventSpec:
    app: str
    event_type: str
    duration_min: int
    title: str
    url: str | None = None
    file_path: str | None = None
    action: str | None = None
    priority: str = "P2"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate richer demo event JSONL")
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--start-date", default="", help="YYYY-MM-DD (local)")
    parser.add_argument("--output", default="logs/demo/demo_events_rich.jsonl")
    parser.add_argument("--tz-offset", default="+09:00")
    parser.add_argument("--cycles", type=int, default=2, help="repeat daily pattern per day")
    return parser.parse_args()


def _tz_from_offset(offset: str) -> timezone:
    try:
        sign = 1 if offset.startswith("+") else -1
        parts = offset.replace("+", "").replace("-", "").split(":")
        hours = int(parts[0])
        minutes = int(parts[1]) if len(parts) > 1 else 0
        return timezone(sign * timedelta(hours=hours, minutes=minutes))
    except Exception:
        return timezone.utc


def _parse_start_date(value: str, tzinfo: timezone) -> datetime:
    if value:
        dt = datetime.strptime(value, "%Y-%m-%d")
        return dt.replace(tzinfo=tzinfo)
    now = datetime.now(tzinfo)
    return datetime(now.year, now.month, now.day, tzinfo=tzinfo)


def _event_id() -> str:
    return str(uuid.uuid4())


def _privacy() -> dict:
    return {"pii_level": "low", "redaction": []}


def _build_event(ts: datetime, spec: EventSpec, resource_id: str) -> dict:
    payload = {
        "duration_sec": max(60, int(spec.duration_min) * 60),
        "window_title": spec.title,
    }
    if spec.url:
        payload["url"] = spec.url
        payload["domain"] = spec.url.split("/")[2] if "://" in spec.url else spec.url
    if spec.file_path:
        payload["file_path"] = spec.file_path
        payload["file_name"] = Path(spec.file_path).name
    if spec.action:
        payload["action"] = spec.action
    return {
        "schema_version": "1.0",
        "event_id": _event_id(),
        "ts": ts.astimezone(timezone.utc).isoformat().replace("+00:00", "Z"),
        "source": "os",
        "app": spec.app,
        "event_type": spec.event_type,
        "priority": spec.priority,
        "resource": {"type": "window", "id": resource_id},
        "payload": payload,
        "privacy": _privacy(),
    }


def _daily_pattern() -> List[EventSpec]:
    return [
        EventSpec(
            app="NOTION.EXE",
            event_type="os.app_focus_block",
            duration_min=25,
            title="Daily Planning - Notion",
        ),
        EventSpec(
            app="CHROME.EXE",
            event_type="browser.tab_active",
            duration_min=5,
            title="Project Spec - Notion",
            url="https://www.notion.so/project-spec",
            action="tab_focus",
        ),
        EventSpec(
            app="CHROME.EXE",
            event_type="os.app_focus_block",
            duration_min=35,
            title="Research - Chrome",
        ),
        EventSpec(
            app="CHROME.EXE",
            event_type="browser.tab_active",
            duration_min=5,
            title="API Docs - Google Docs",
            url="https://docs.google.com/document/d/1-demo",
            action="tab_focus",
        ),
        EventSpec(
            app="CODE.EXE",
            event_type="os.app_focus_block",
            duration_min=60,
            title="Implementation - VS Code",
        ),
        EventSpec(
            app="CODE.EXE",
            event_type="os.file_saved",
            duration_min=2,
            title="Save: workflow_builder.py",
            file_path="C:\\projects\\demo\\workflow_builder.py",
            action="file_save",
            priority="P0",
        ),
        EventSpec(
            app="CHROME.EXE",
            event_type="browser.tab_active",
            duration_min=5,
            title="PR Review - GitHub",
            url="https://github.com/demo/repo/pull/12",
            action="tab_focus",
        ),
        EventSpec(
            app="NOTION.EXE",
            event_type="os.app_focus_block",
            duration_min=20,
            title="Notes - Notion",
        ),
        EventSpec(
            app="KAKAOTALK.EXE",
            event_type="os.app_focus_block",
            duration_min=10,
            title="Team Chat - KakaoTalk",
        ),
        EventSpec(
            app="CHROME.EXE",
            event_type="upload_done",
            duration_min=2,
            title="Upload Report - Chrome",
            url="https://drive.google.com/drive/folders/demo",
            action="upload",
            priority="P0",
        ),
    ]


def main() -> None:
    args = parse_args()
    tzinfo = _tz_from_offset(args.tz_offset)
    start_date = _parse_start_date(args.start_date, tzinfo)

    lines: List[str] = []
    day_count = max(1, int(args.days))
    cycles = max(1, int(args.cycles))
    base_pattern = _daily_pattern()

    for day in range(day_count):
        base = start_date + timedelta(days=day)
        for cycle in range(cycles):
            cursor = base.replace(hour=9, minute=0, second=0, microsecond=0) + timedelta(
                minutes=cycle * 120
            )
            for idx, spec in enumerate(base_pattern):
                resource_id = f"demo-{spec.app.lower()}-{day}-{cycle}-{idx}"
                event = _build_event(cursor, spec, resource_id)
                lines.append(json.dumps(event, ensure_ascii=False))
                cursor += timedelta(minutes=max(3, spec.duration_min // 3))

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"demo_events_saved={out_path} count={len(lines)}")


if __name__ == "__main__":
    main()
