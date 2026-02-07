from __future__ import annotations

import argparse
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate workflow with scoring retry loop")
    parser.add_argument("--input", required=True, help="llm_input path")
    parser.add_argument("--config", required=True, help="config path")
    parser.add_argument("--output", required=True, help="workflow output path")
    parser.add_argument("--profile", default="", help="profile json path")
    parser.add_argument("--score-config", default="", help="score config path")
    parser.add_argument("--min-score", type=int, default=85)
    parser.add_argument("--max-attempts", type=int, default=3)
    parser.add_argument("--log", default="logs/workflow_improve.log")
    return parser.parse_args()


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        return {}
    return json.loads(p.read_text(encoding="utf-8"))


def _write_json(path: Path, data: dict) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def _score(file_path: str, profile: str, score_config: str) -> tuple[int, list[str]]:
    cmd = ["python", "scripts/score_n8n_workflow.py", "--file", file_path]
    if profile:
        cmd += ["--profile", profile]
    if score_config:
        cmd += ["--score-config", score_config]
    result = subprocess.run(cmd, capture_output=True, text=True)
    score = 0
    notes = []
    for line in result.stdout.splitlines():
        if line.startswith("score="):
            try:
                score = int(line.split("=")[1].strip())
            except Exception:
                pass
        if line.startswith("- "):
            notes.append(line[2:].strip())
    return score, notes


def main() -> None:
    args = parse_args()

    profile_path = Path(args.profile) if args.profile else None
    base_profile = _load_json(str(profile_path)) if profile_path else {}

    log_path = Path(args.log)
    log_path.parent.mkdir(parents=True, exist_ok=True)

    for attempt in range(1, args.max_attempts + 1):
        # Write a temp profile with feedback
        temp_profile = dict(base_profile)
        if attempt > 1:
            temp_profile["quality_feedback"] = {
                "attempt": attempt,
                "notes": last_notes,
                "min_score": args.min_score,
            }
        temp_profile_path = Path("logs/profile_runtime.json")
        _write_json(temp_profile_path, temp_profile)

        cmd = [
            "python",
            "scripts/generate_n8n_workflow.py",
            "--config",
            args.config,
            "--input",
            args.input,
            "--output",
            args.output,
            "--profile",
            str(temp_profile_path),
        ]
        subprocess.run(cmd, check=False)

        score, notes = _score(args.output, str(temp_profile_path), args.score_config)
        entry = {
            "ts": _now_utc(),
            "attempt": attempt,
            "score": score,
            "notes": notes,
            "output": args.output,
        }
        with log_path.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(entry, ensure_ascii=False) + "\n")

        if score >= args.min_score:
            print(f"success score={score} attempts={attempt}")
            return

        last_notes = notes

    print(f"failed to reach min_score={args.min_score} after {args.max_attempts} attempts")


if __name__ == "__main__":
    main()
