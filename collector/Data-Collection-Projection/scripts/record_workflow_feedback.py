from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Record workflow feedback (approve/reject)")
    parser.add_argument("--id", required=True, help="workflow fingerprint id")
    parser.add_argument("--status", choices=["approved", "rejected"], required=True)
    parser.add_argument("--file", default="configs/workflow_feedback.json", help="feedback json path")
    parser.add_argument("--source", default="manual", help="feedback source (manual/auto)")
    parser.add_argument("--score", default="", help="score at time of feedback")
    parser.add_argument("--tools", default="", help="comma-separated tools")
    parser.add_argument("--reasons", default="", help="semicolon-separated reasons")
    parser.add_argument(
        "--history",
        default="logs/workflow_feedback_history.jsonl",
        help="history log jsonl",
    )
    return parser.parse_args()


def _load_json(path: Path) -> dict:
    if not path.exists():
        return {"approved": [], "rejected": [], "weights": {"approved_bonus": 15, "rejected_penalty": -30}}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {"approved": [], "rejected": [], "weights": {"approved_bonus": 15, "rejected_penalty": -30}}


def _save(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def _append_history(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(payload, ensure_ascii=False) + "\n")


def main() -> None:
    args = parse_args()
    project_root = Path(__file__).resolve().parents[1]
    path = Path(args.file)
    if not path.is_absolute():
        path = project_root / path
    data = _load_json(path)
    approved = set(data.get("approved") or [])
    rejected = set(data.get("rejected") or [])

    if args.status == "approved":
        approved.add(args.id)
        rejected.discard(args.id)
    else:
        rejected.add(args.id)
        approved.discard(args.id)

    data["approved"] = sorted(approved)
    data["rejected"] = sorted(rejected)
    _save(path, data)

    history_path = Path(args.history)
    if not history_path.is_absolute():
        history_path = project_root / history_path
    tools = [t.strip() for t in str(args.tools).split(",") if t.strip()]
    reasons = [r.strip() for r in str(args.reasons).split(";") if r.strip()]
    payload = {
        "ts": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "id": args.id,
        "status": args.status,
        "source": args.source,
        "score": args.score,
        "tools": tools,
        "reasons": reasons,
    }
    _append_history(history_path, payload)
    print(f"feedback_saved={path}")


if __name__ == "__main__":
    main()
