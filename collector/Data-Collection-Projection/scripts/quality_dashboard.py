from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate quality dashboard summary")
    parser.add_argument("--log-dir", default="logs", help="log directory")
    parser.add_argument("--feedback", default="configs/workflow_feedback.json", help="feedback json path")
    parser.add_argument(
        "--history",
        default="logs/workflow_feedback_history.jsonl",
        help="feedback history jsonl",
    )
    return parser.parse_args()


def _load_json(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _load_json_list(path: Path) -> list[dict]:
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return data if isinstance(data, list) else []
    except Exception:
        return []


def _load_history(path: Path) -> list[dict]:
    if not path.exists():
        return []
    items = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            items.append(json.loads(line))
        except Exception:
            continue
    return items


def _load_events(path: Path) -> list[dict]:
    if not path.exists():
        return []
    items = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            items.append(json.loads(line))
        except Exception:
            continue
    return items


def _summarize_scores(recs: list[dict]) -> dict[str, Any]:
    scores = []
    for item in recs:
        if isinstance(item, dict):
            score = item.get("final_score") if item.get("final_score") is not None else item.get("base_score")
            if isinstance(score, (int, float)):
                scores.append(float(score))
    if not scores:
        return {"count": 0}
    return {
        "count": len(scores),
        "min": min(scores),
        "max": max(scores),
        "avg": round(sum(scores) / len(scores), 2),
    }


def _tools_stats(recs: list[dict]) -> dict[str, int]:
    stats: dict[str, int] = {}
    for item in recs:
        tools = item.get("tools") if isinstance(item, dict) else None
        if isinstance(tools, list):
            for tool in tools:
                key = str(tool).lower()
                stats[key] = stats.get(key, 0) + 1
    return stats


def _import_stats(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"count": 0}
    total = 0
    ok = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            data = json.loads(line)
        except Exception:
            continue
        status = data.get("status")
        total += 1
        if isinstance(status, int) and 200 <= status < 300:
            ok += 1
    return {"count": total, "success": ok, "success_rate": round(ok / total, 3) if total else 0}


def _feedback_stats(history: list[dict]) -> dict[str, Any]:
    if not history:
        return {"count": 0}
    total = len(history)
    approved = sum(1 for item in history if item.get("status") == "approved")
    rejected = sum(1 for item in history if item.get("status") == "rejected")
    auto = sum(1 for item in history if item.get("source") == "auto")
    return {
        "count": total,
        "approved": approved,
        "rejected": rejected,
        "approval_rate": round(approved / total, 3) if total else 0,
        "auto_approval_rate": round(auto / total, 3) if total else 0,
    }


def _event_counts(events: list[dict]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for event in events:
        etype = event.get("type")
        if not etype:
            continue
        counts[etype] = counts.get(etype, 0) + 1
    return counts


def _auto_approval_reasons(events: list[dict]) -> dict[str, int]:
    reasons: dict[str, int] = {}
    for event in events:
        if event.get("type") != "auto_approved":
            continue
        for reason in event.get("reasons") or []:
            key = str(reason)
            reasons[key] = reasons.get(key, 0) + 1
    return reasons


def _reason_counts(events: list[dict], event_type: str, field: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for event in events:
        if event.get("type") != event_type:
            continue
        value = event.get(field)
        if value is None or value == "":
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return counts


def main() -> None:
    args = parse_args()
    project_root = Path(__file__).resolve().parents[1]
    log_dir = Path(args.log_dir)
    if not log_dir.is_absolute():
        log_dir = project_root / log_dir
    feedback_path = Path(args.feedback)
    if not feedback_path.is_absolute():
        feedback_path = project_root / feedback_path
    history_path = Path(args.history)
    if not history_path.is_absolute():
        history_path = project_root / history_path
    recs = _load_json_list(log_dir / "workflow_recommendations.json")
    candidates = _load_json_list(log_dir / "workflow_candidates.json")
    feedback = _load_json(feedback_path)
    history = _load_history(history_path)
    llm_input = _load_json(log_dir / "llm_input_hybrid.json")
    events = _load_events(log_dir / "quality_events.jsonl")
    event_counts = _event_counts(events)
    auto_reasons = _auto_approval_reasons(events)
    llm_error_reasons = _reason_counts(events, "llm_error", "reason")
    import_failure_codes = _reason_counts(events, "import_failed", "code")

    summary = {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "recommendations": {
            "scores": _summarize_scores(recs),
            "tools": _tools_stats(recs),
            "count": len(recs),
        },
        "candidates": {"count": len(candidates)},
        "feedback": {
            "approved_count": len(feedback.get("approved") or []),
            "rejected_count": len(feedback.get("rejected") or []),
            "history": _feedback_stats(history),
        },
        "import": _import_stats(log_dir / "n8n_import.log"),
        "input_quality": {
            "total_events": (llm_input.get("quality") or {}).get("total_events"),
            "active_minutes": (llm_input.get("quality") or {}).get("active_minutes"),
        },
        "events": event_counts,
        "auto_approval_reasons": auto_reasons,
        "llm_error_reasons": llm_error_reasons,
        "import_failure_codes": import_failure_codes,
        "failures": {
            "no_candidates": event_counts.get("no_candidates", 0),
            "recommendations_failed": event_counts.get("recommendations_failed", 0),
            "recommendations_parse_failed": event_counts.get("recommendations_parse_failed", 0),
            "import_failed": event_counts.get("import_failed", 0),
        },
    }

    out_json = log_dir / "quality_dashboard.json"
    out_json.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")

    lines = [
        "# Quality Dashboard",
        "",
        f"- generated_at: {summary['generated_at']}",
        f"- recommendations: {summary['recommendations']['count']}",
        f"- candidates: {summary['candidates']['count']}",
        f"- scores: {summary['recommendations']['scores']}",
        f"- tools: {summary['recommendations']['tools']}",
        f"- feedback: {summary['feedback']['history']}",
        f"- import: {summary['import']}",
        f"- input_quality: {summary['input_quality']}",
        f"- failures: {summary['failures']}",
        f"- auto_approval_reasons: {summary['auto_approval_reasons']}",
        f"- llm_error_reasons: {summary['llm_error_reasons']}",
        f"- import_failure_codes: {summary['import_failure_codes']}",
        f"- events: {summary['events']}",
        "",
    ]
    (log_dir / "quality_dashboard.md").write_text("\n".join(lines), encoding="utf-8")
    print(f"quality_dashboard_saved={out_json}")


if __name__ == "__main__":
    main()
