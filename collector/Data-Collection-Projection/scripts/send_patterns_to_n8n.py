from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable
from urllib import request


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Send pattern hits to n8n webhook")
    parser.add_argument(
        "--pattern",
        default="",
        help="pattern_summary.json path",
    )
    parser.add_argument(
        "--webhook",
        default="",
        help="n8n webhook URL",
    )
    parser.add_argument("--min-confidence", type=float, default=0.7)
    parser.add_argument("--min-support", type=int, default=3)
    parser.add_argument("--min-days", type=int, default=3)
    parser.add_argument("--max-events", type=int, default=10)
    parser.add_argument(
        "--types",
        default="hourly,sequence,transition,bucket",
        help="comma-separated types: hourly,sequence,transition,bucket",
    )
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--output", default="", help="optional output jsonl")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if not args.pattern:
        print("pattern_summary path is required")
        sys.exit(1)
    pattern_path = Path(args.pattern)
    if not pattern_path.exists():
        print(f"pattern_summary not found: {pattern_path}")
        sys.exit(1)

    payload = _load_json(pattern_path)
    types = {item.strip().lower() for item in args.types.split(",") if item.strip()}

    events: list[dict[str, Any]] = []
    if "hourly" in types:
        events.extend(_build_hourly(payload, args))
    if "sequence" in types:
        events.extend(_build_sequences(payload, args))
    if "transition" in types:
        events.extend(_build_transitions(payload, args))
    if "bucket" in types:
        events.extend(_build_buckets(payload, args))

    events = events[: max(0, int(args.max_events))]
    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(
            "\n".join(json.dumps(item, ensure_ascii=False) for item in events) + "\n",
            encoding="utf-8",
        )
        print(f"events_saved={out_path}")

    if args.dry_run or not args.webhook:
        print(json.dumps({"count": len(events), "events": events}, ensure_ascii=False, indent=2))
        return

    sent = 0
    for event in events:
        if _post_json(args.webhook, event):
            sent += 1
    print(json.dumps({"sent": sent, "total": len(events)}, ensure_ascii=False))


def _load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _build_hourly(payload: dict, args: argparse.Namespace) -> list[dict]:
    results = []
    for item in payload.get("patterns") or []:
        if item.get("confidence", 0) < args.min_confidence:
            continue
        if int(item.get("days", 0)) < args.min_days:
            continue
        results.append(
            {
                "event": "pattern_hit",
                "type": "hourly",
                "pattern": [item.get("app")],
                "confidence": item.get("confidence", 0.0),
                "time_window": f"{int(item.get('hour', 0)):02d}",
                "minutes": int(item.get("minutes", 0)),
                "days": int(item.get("days", 0)),
                "generated_at": _now(),
            }
        )
    return results


def _build_sequences(payload: dict, args: argparse.Namespace) -> list[dict]:
    results = []
    for item in payload.get("sequence_patterns") or []:
        if item.get("confidence", 0) < args.min_confidence:
            continue
        if int(item.get("support", 0)) < args.min_support:
            continue
        results.append(
            {
                "event": "pattern_hit",
                "type": "sequence",
                "pattern": item.get("sequence") or [],
                "confidence": item.get("confidence", 0.0),
                "support": int(item.get("support", 0)),
                "generated_at": _now(),
            }
        )
    return results


def _build_transitions(payload: dict, args: argparse.Namespace) -> list[dict]:
    results = []
    for item in payload.get("transition_patterns") or []:
        if int(item.get("support", 0)) < args.min_support:
            continue
        results.append(
            {
                "event": "pattern_hit",
                "type": "transition",
                "pattern": [item.get("from"), item.get("to")],
                "confidence": _support_confidence(item.get("support", 0)),
                "support": int(item.get("support", 0)),
                "generated_at": _now(),
            }
        )
    return results


def _build_buckets(payload: dict, args: argparse.Namespace) -> list[dict]:
    results = []
    buckets = payload.get("time_bucket_patterns") or {}
    for bucket, item in buckets.items():
        if int(item.get("days", 0)) < args.min_days:
            continue
        results.append(
            {
                "event": "pattern_hit",
                "type": "bucket",
                "pattern": [item.get("app")],
                "confidence": _support_confidence(item.get("days", 0)),
                "time_window": bucket,
                "minutes": int(item.get("minutes", 0)),
                "days": int(item.get("days", 0)),
                "generated_at": _now(),
            }
        )
    return results


def _support_confidence(value: int) -> float:
    return round(min(1.0, max(0, value) / 10.0), 3)


def _post_json(url: str, payload: dict) -> bool:
    data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with request.urlopen(req, timeout=5) as resp:
            return 200 <= resp.status < 300
    except Exception:
        return False


def _now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    main()
