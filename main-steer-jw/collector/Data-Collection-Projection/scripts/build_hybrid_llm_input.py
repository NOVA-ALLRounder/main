from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build hybrid LLM input (realtime + summaries)")
    parser.add_argument("--config", default="", help="optional config path")
    parser.add_argument("--daily", default="", help="daily_summary.json path")
    parser.add_argument("--pattern", default="", help="pattern_summary.json path")
    parser.add_argument("--realtime", default="logs/llm_input_realtime.json", help="realtime llm input path")
    parser.add_argument("--output", default="logs/llm_input_hybrid.json")
    parser.add_argument("--max-bytes", type=int, default=8000)
    return parser.parse_args()


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _load_json(path: str) -> dict:
    if not path:
        return {}
    p = Path(path)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _latest_daily(log_dir: Path) -> str:
    files = sorted(log_dir.glob("daily_summary_*.json"))
    return str(files[-1]) if files else ""


def _compress_payload(payload: dict, max_bytes: int) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(val: dict) -> int:
        return len(json.dumps(val, ensure_ascii=False).encode("utf-8"))

    if _size(payload) <= max_bytes:
        return payload

    compact = dict(payload)
    compact["recent_events"] = (compact.get("recent_events") or [])[:5]
    if _size(compact) <= max_bytes:
        return compact

    compact["recent_sequence"] = (compact.get("recent_sequence") or [])[:6]
    compact["app_transitions"] = (compact.get("app_transitions") or [])[:4]
    compact["key_event_tokens"] = (compact.get("key_event_tokens") or [])[:8]
    if _size(compact) <= max_bytes:
        return compact

    compact["top_titles"] = []
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = (compact.get("top_apps") or [])[:3]
    compact["hourly_patterns"] = (compact.get("hourly_patterns") or [])[:3]
    compact["sequence_patterns"] = (compact.get("sequence_patterns") or [])[:2]
    compact["transition_patterns"] = (compact.get("transition_patterns") or [])[:2]
    compact["focus_blocks"] = (compact.get("focus_blocks") or [])[:3]
    if _size(compact) <= max_bytes:
        return compact

    compact["hourly_patterns"] = []
    compact["sequence_patterns"] = []
    compact["transition_patterns"] = []
    compact["weekday_patterns"] = {}
    compact["time_bucket_patterns"] = {}
    compact["focus_block_stats"] = {}
    compact["key_events"] = {}
    compact["recent_events"] = []
    compact["recent_sequence"] = []
    compact["app_transitions"] = []
    compact["key_event_tokens"] = []
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def main() -> None:
    args = parse_args()
    log_dir = Path(args.output).resolve().parent

    daily_path = args.daily or _latest_daily(log_dir)
    pattern_path = args.pattern or str(log_dir / "pattern_summary.json")

    daily = _load_json(daily_path)
    pattern = _load_json(pattern_path)
    realtime = _load_json(args.realtime)

    payload = {
        "generated_at": _now_utc(),
        "window": realtime.get("window"),
        "date_local": daily.get("date_local") or realtime.get("date_local"),
        "quality": realtime.get("quality") or {},
        "top_apps": realtime.get("top_apps") or daily.get("top_apps") or [],
        "top_titles": realtime.get("top_titles") or daily.get("top_titles") or [],
        "focus_blocks": realtime.get("focus_blocks") or [],
        "key_events": realtime.get("key_events") or daily.get("key_events") or {},
        "key_event_tokens": realtime.get("key_event_tokens") or [],
        "app_transitions": realtime.get("app_transitions") or daily.get("top_transitions") or [],
        "recent_sequence": realtime.get("recent_sequence") or [],
        "recent_events": realtime.get("recent_events") or [],
        "intent_candidates": realtime.get("intent_candidates") or daily.get("intent_candidates") or [],
        "intent_summary": realtime.get("intent_summary") or daily.get("intent_summary") or {},
        "workflow_hints": realtime.get("workflow_hints") or daily.get("workflow_hints") or {},
        "sequence_signature": realtime.get("sequence_signature")
        or daily.get("sequence_signature")
        or "",
        "hourly_patterns": pattern.get("patterns") or [],
        "weekday_patterns": pattern.get("weekday_patterns") or {},
        "sequence_patterns": pattern.get("sequence_patterns") or [],
        "transition_patterns": pattern.get("transition_patterns") or [],
        "time_bucket_patterns": pattern.get("time_bucket_patterns") or {},
        "focus_block_stats": pattern.get("focus_block_stats") or {},
        "source": "hybrid",
        "notes": [
            "Hybrid input: realtime + recent summaries.",
            f"daily={Path(daily_path).name if daily_path else ''}",
            f"pattern={Path(pattern_path).name if pattern_path else ''}",
        ],
    }

    payload = _compress_payload(payload, args.max_bytes)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"hybrid_llm_input_saved={out_path}")


if __name__ == "__main__":
    main()
