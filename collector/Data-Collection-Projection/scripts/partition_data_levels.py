from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SENSITIVE_KEYS = {
    "window_title",
    "title",
    "document_title",
    "tab_title",
    "page_title",
    "url",
    "page_url",
    "query",
    "search_query",
    "content",
    "content_summary",
    "text",
    "body",
    "html",
    "clipboard",
    "path",
    "file_path",
    "filepath",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Partition logs into data levels")
    parser.add_argument("--log-dir", default="logs", help="log directory")
    return parser.parse_args()


def _sanitize(obj: Any) -> Any:
    if isinstance(obj, dict):
        out = {}
        for key, value in obj.items():
            if str(key).lower() in SENSITIVE_KEYS:
                continue
            out[key] = _sanitize(value)
        return out
    if isinstance(obj, list):
        return [_sanitize(item) for item in obj[:50]]
    if isinstance(obj, str):
        return obj[:256]
    return obj


def _copy_if_exists(src: Path, dst: Path) -> None:
    if not src.exists():
        return
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_bytes(src.read_bytes())


def main() -> None:
    args = parse_args()
    log_dir = Path(args.log_dir)
    raw_dir = log_dir / "raw"
    minimal_dir = log_dir / "minimal"
    agg_dir = log_dir / "aggregated"

    # Raw logs (as-is)
    for name in ["collector.log", "activity_detail.log", "activity_detail.txt"]:
        _copy_if_exists(log_dir / name, raw_dir / name)

    # Aggregated outputs
    for name in [
        "workflow_recommendations.json",
        "workflow_recommendations.md",
        "workflow_candidates.json",
        "n8n_workflow.json",
        "quality_dashboard.json",
        "quality_dashboard.md",
        "pattern_summary.json",
    ]:
        _copy_if_exists(log_dir / name, agg_dir / name)

    # Minimal (sanitized)
    for name in [
        "llm_input_realtime.json",
        "llm_input_hybrid.json",
        "pattern_summary.json",
    ]:
        src = log_dir / name
        if not src.exists():
            continue
        try:
            data = json.loads(src.read_text(encoding="utf-8"))
        except Exception:
            continue
        sanitized = _sanitize(data)
        out = minimal_dir / name
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(sanitized, ensure_ascii=False, indent=2), encoding="utf-8")

    manifest = {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "raw_dir": str(raw_dir),
        "minimal_dir": str(minimal_dir),
        "aggregated_dir": str(agg_dir),
    }
    (log_dir / "partition_manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print("partition_completed")


if __name__ == "__main__":
    main()
