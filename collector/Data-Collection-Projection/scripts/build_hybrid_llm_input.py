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
    parser.add_argument(
        "--compression-level",
        default="balanced",
        choices=["strict", "balanced", "permissive"],
        help="compression level",
    )
    parser.add_argument(
        "--preserve-fields",
        default="",
        help="comma-separated fields to preserve during compression",
    )
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
    return _compress_payload_with_level(payload, max_bytes, "balanced", set())


def _parse_preserve_fields(raw: str) -> set[str]:
    if not raw:
        return set()
    return {item.strip().lower() for item in raw.split(",") if item.strip()}


def _default_preserve_fields(level: str) -> set[str]:
    level = (level or "").lower().strip()
    if level == "strict":
        return {"top_domains", "recent_sequence", "app_transitions"}
    if level == "permissive":
        return {
            "top_titles",
            "recent_events",
            "top_domains",
            "top_pages",
            "focus_blocks",
            "recent_sequence",
            "app_transitions",
            "key_event_tokens",
            "hourly_patterns",
            "sequence_patterns",
            "transition_patterns",
        }
    return {
        "top_domains",
        "top_pages",
        "recent_sequence",
        "app_transitions",
        "key_event_tokens",
        "sequence_patterns",
        "transition_patterns",
    }


def _prune_list(value: list, limit: int) -> list:
    if limit <= 0:
        return []
    return value[:limit]


def _compress_payload_with_level(payload: dict, max_bytes: int, level: str, preserve: set[str]) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(val: dict) -> int:
        return len(json.dumps(val, ensure_ascii=False).encode("utf-8"))

    preserve_browser = "browser_context" in preserve
    preserve_domains = preserve_browser or "top_domains" in preserve
    preserve_pages = preserve_browser or "top_pages" in preserve
    preserve_transitions = "app_transitions" in preserve or "transitions" in preserve
    preserve_sequence = "recent_sequence" in preserve or "sequence" in preserve
    preserve_tokens = "key_event_tokens" in preserve or "tokens" in preserve
    preserve_focus = "focus_blocks" in preserve or "focus" in preserve
    preserve_hourly = "hourly_patterns" in preserve
    preserve_seq_patterns = "sequence_patterns" in preserve
    preserve_trans_patterns = "transition_patterns" in preserve

    compact = dict(payload)

    if level == "strict":
        if "top_titles" not in preserve:
            compact["top_titles"] = []
        if "recent_events" not in preserve:
            compact["recent_events"] = []
        browser_context = compact.get("browser_context") or {}
        if isinstance(browser_context, dict):
            browser_context = dict(browser_context)
            if not preserve_pages:
                browser_context["top_pages"] = []
            if not preserve_domains:
                browser_context["top_domains"] = _prune_list(browser_context.get("top_domains") or [], 3)
            compact["browser_context"] = browser_context
        if not preserve_transitions:
            compact["app_transitions"] = _prune_list(compact.get("app_transitions") or [], 4)
        if not preserve_sequence:
            compact["recent_sequence"] = _prune_list(compact.get("recent_sequence") or [], 6)
        if not preserve_tokens:
            compact["key_event_tokens"] = _prune_list(compact.get("key_event_tokens") or [], 8)
        if not preserve_focus:
            compact["focus_blocks"] = _prune_list(compact.get("focus_blocks") or [], 3)
        if not preserve_hourly:
            compact["hourly_patterns"] = _prune_list(compact.get("hourly_patterns") or [], 3)
        if not preserve_seq_patterns:
            compact["sequence_patterns"] = _prune_list(compact.get("sequence_patterns") or [], 2)
        if not preserve_trans_patterns:
            compact["transition_patterns"] = _prune_list(compact.get("transition_patterns") or [], 2)
    elif level == "balanced":
        if "recent_events" not in preserve:
            compact["recent_events"] = _prune_list(compact.get("recent_events") or [], 5)
        if "top_titles" not in preserve:
            compact["top_titles"] = _prune_list(compact.get("top_titles") or [], 5)
        browser_context = compact.get("browser_context") or {}
        if isinstance(browser_context, dict):
            browser_context = dict(browser_context)
            if not preserve_pages:
                browser_context["top_pages"] = _prune_list(browser_context.get("top_pages") or [], 3)
            if not preserve_domains:
                browser_context["top_domains"] = _prune_list(browser_context.get("top_domains") or [], 5)
            compact["browser_context"] = browser_context
    elif level == "permissive":
        pass

    if _size(compact) <= max_bytes:
        return compact

    if "recent_events" not in preserve:
        compact["recent_events"] = _prune_list(compact.get("recent_events") or [], 5)
    browser_context = compact.get("browser_context") or {}
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_pages:
            browser_context["top_pages"] = _prune_list(browser_context.get("top_pages") or [], 3)
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    if not preserve_sequence:
        compact["recent_sequence"] = _prune_list(compact.get("recent_sequence") or [], 6)
    if not preserve_transitions:
        compact["app_transitions"] = _prune_list(compact.get("app_transitions") or [], 4)
    if not preserve_tokens:
        compact["key_event_tokens"] = _prune_list(compact.get("key_event_tokens") or [], 8)
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_pages:
            browser_context["top_pages"] = []
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    if "top_titles" not in preserve:
        compact["top_titles"] = []
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_domains:
            browser_context["top_domains"] = []
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = _prune_list(compact.get("top_apps") or [], 3)
    if not preserve_hourly:
        compact["hourly_patterns"] = _prune_list(compact.get("hourly_patterns") or [], 3)
    if not preserve_seq_patterns:
        compact["sequence_patterns"] = _prune_list(compact.get("sequence_patterns") or [], 2)
    if not preserve_trans_patterns:
        compact["transition_patterns"] = _prune_list(compact.get("transition_patterns") or [], 2)
    if not preserve_focus:
        compact["focus_blocks"] = _prune_list(compact.get("focus_blocks") or [], 3)
    if _size(compact) <= max_bytes:
        return compact

    if not preserve_hourly:
        compact["hourly_patterns"] = []
    if not preserve_seq_patterns:
        compact["sequence_patterns"] = []
    if not preserve_trans_patterns:
        compact["transition_patterns"] = []
    compact["weekday_patterns"] = {}
    compact["time_bucket_patterns"] = {}
    compact["focus_block_stats"] = {}
    compact["key_events"] = {}
    if "recent_events" not in preserve:
        compact["recent_events"] = []
    if not preserve_sequence:
        compact["recent_sequence"] = []
    if not preserve_transitions:
        compact["app_transitions"] = []
    if not preserve_tokens:
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

    preserve_fields = _parse_preserve_fields(args.preserve_fields)
    preserve_fields |= _default_preserve_fields(args.compression_level)
    payload = {
        "generated_at": _now_utc(),
        "window": realtime.get("window"),
        "date_local": daily.get("date_local") or realtime.get("date_local"),
        "time_context": realtime.get("time_context") or daily.get("time_context") or {},
        "quality": realtime.get("quality") or {},
        "top_apps": realtime.get("top_apps") or daily.get("top_apps") or [],
        "top_titles": realtime.get("top_titles") or daily.get("top_titles") or [],
        "browser_context": realtime.get("browser_context") or {},
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
        "task_units": realtime.get("task_units") or daily.get("task_units") or [],
        "task_summary": realtime.get("task_summary") or daily.get("task_summary") or {},
        "pattern_repetition": realtime.get("pattern_repetition") or daily.get("pattern_repetition") or {},
        "auto_approve": realtime.get("auto_approve") or daily.get("auto_approve") or False,
        "hourly_patterns": pattern.get("patterns") or [],
        "weekday_patterns": pattern.get("weekday_patterns") or {},
        "sequence_patterns": pattern.get("sequence_patterns") or [],
        "transition_patterns": pattern.get("transition_patterns") or [],
        "time_bucket_patterns": pattern.get("time_bucket_patterns") or {},
        "focus_block_stats": pattern.get("focus_block_stats") or {},
        "compression": {
            "level": args.compression_level,
            "max_bytes": args.max_bytes,
            "preserve_fields": sorted(preserve_fields),
        },
        "source": "hybrid",
        "notes": [
            "Hybrid input: realtime + recent summaries.",
            f"daily={Path(daily_path).name if daily_path else ''}",
            f"pattern={Path(pattern_path).name if pattern_path else ''}",
        ],
    }

    payload = _compress_payload_with_level(payload, args.max_bytes, args.compression_level, preserve_fields)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"hybrid_llm_input_saved={out_path}")


if __name__ == "__main__":
    main()
