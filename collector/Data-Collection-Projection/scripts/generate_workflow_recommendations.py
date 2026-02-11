from __future__ import annotations

import argparse
import json
import os
import subprocess
import hashlib
from pathlib import Path
from typing import Any, List
import sys


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate and rank workflow candidates")
    parser.add_argument("--config", required=True, help="config path")
    parser.add_argument("--input", required=True, help="llm_input path")
    parser.add_argument("--profile", default="", help="profile json path")
    parser.add_argument("--score-config", default="", help="score config path")
    parser.add_argument("--output-dir", default="logs", help="output directory")
    parser.add_argument("--candidates", type=int, default=3, help="LLM candidates")
    parser.add_argument("--include-template", action="store_true", help="include template fallback candidate")
    parser.add_argument("--min-score", type=int, default=85)
    parser.add_argument("--pattern", default="", help="pattern_summary json path")
    parser.add_argument("--feedback", default="configs/workflow_feedback.json", help="feedback json path")
    parser.add_argument("--save-best", default="logs/n8n_workflow.json", help="path to save best workflow")
    return parser.parse_args()


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def _load_quality_events(log_dir: Path, max_events: int = 200) -> list[dict]:
    path = log_dir / "quality_events.jsonl"
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    if len(lines) > max_events:
        lines = lines[-max_events:]
    events: list[dict] = []
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except Exception:
            continue
    return events


def _summarize_failures(events: list[dict]) -> dict[str, Any]:
    summary: dict[str, Any] = {"by_type": {}, "llm_error_reasons": {}, "import_failure_codes": {}}
    for event in events:
        etype = event.get("type")
        if not etype:
            continue
        summary["by_type"][etype] = summary["by_type"].get(etype, 0) + 1
        if etype == "llm_error":
            reason = event.get("reason") or "unknown"
            summary["llm_error_reasons"][reason] = summary["llm_error_reasons"].get(reason, 0) + 1
        if etype == "import_failed":
            code = str(event.get("code") or "unknown")
            summary["import_failure_codes"][code] = summary["import_failure_codes"].get(code, 0) + 1
    return summary


def _score(file_path: str, profile: str, score_config: str, llm_input: str) -> tuple[int, list[str]]:
    cmd = [sys.executable, str(PROJECT_ROOT / "scripts" / "score_n8n_workflow.py"), "--file", file_path]
    if profile:
        cmd += ["--profile", profile]
    if score_config:
        cmd += ["--score-config", score_config]
    if llm_input:
        cmd += ["--llm-input", llm_input]
    result = subprocess.run(cmd, capture_output=True, text=True, cwd=PROJECT_ROOT)
    score = 0
    notes: list[str] = []
    for line in result.stdout.splitlines():
        if line.startswith("score="):
            try:
                score = int(line.split("=")[1].strip())
            except Exception:
                pass
        if line.startswith("- "):
            notes.append(line[2:].strip())
    return score, notes


def _fingerprint(payload: dict) -> str:
    nodes = payload.get("nodes") or []
    connections = payload.get("connections") or {}
    blob = json.dumps({"nodes": nodes, "connections": connections}, ensure_ascii=False, sort_keys=True)
    return hashlib.sha256(blob.encode("utf-8")).hexdigest()[:12]


def _tools_in_workflow(payload: dict) -> list[str]:
    nodes = payload.get("nodes") or []
    tools = set()
    for node in nodes:
        if not isinstance(node, dict):
            continue
        t = str(node.get("type", "")).lower()
        if "notion" in t:
            tools.add("notion")
        if "slack" in t:
            tools.add("slack")
        if "gmail" in t:
            tools.add("gmail")
        if "googlecalendar" in t:
            tools.add("googlecalendar")
        if "http" in t and "kakao" in str(node.get("name", "")).lower():
            tools.add("kakao")
    return sorted(tools)


def _load_feedback(path: str) -> dict:
    data = _load_json(path)
    if not isinstance(data, dict):
        return {}
    return data


def _feedback_adjustment(fid: str, tools: list[str], feedback: dict) -> tuple[int, list[str]]:
    bonus = 0
    reasons: list[str] = []
    approved = set(feedback.get("approved") or [])
    rejected = set(feedback.get("rejected") or [])
    weights = feedback.get("weights") or {}
    approved_bonus = int(weights.get("approved_bonus", 15))
    rejected_penalty = int(weights.get("rejected_penalty", -30))
    tool_bonus = feedback.get("tool_bonus") or {"notion": 2, "slack": 2, "gmail": 1}

    pinned = False
    excluded = False

    if fid in approved:
        bonus += approved_bonus
        reasons.append("approved_before")
        pinned = True
    if fid in rejected:
        bonus += rejected_penalty
        reasons.append("rejected_before")
        excluded = True

    for tool in tools:
        try:
            bonus += int(tool_bonus.get(tool, 0))
        except Exception:
            pass
    return bonus, reasons, pinned, excluded


def _load_pattern(path: str, output_dir: Path) -> dict:
    if path:
        return _load_json(path)
    candidate = output_dir / "pattern_summary.json"
    return _load_json(str(candidate))


def _short_sequence(seq: list[str], max_len: int = 5) -> str:
    if not seq:
        return ""
    return " -> ".join(seq[:max_len])


def _build_rationale(llm_input: dict, pattern: dict, tools: list[str]) -> list[str]:
    reasons: list[str] = []
    if not isinstance(llm_input, dict):
        return reasons

    intent = (llm_input.get("intent_summary") or {}).get("primary_intent")
    if intent:
        reasons.append(f"intent:{intent}")

    task = (llm_input.get("task_summary") or {}).get("primary_task")
    if task:
        reasons.append(f"task:{task}")

    hints = llm_input.get("workflow_hints") or {}
    if isinstance(hints, dict):
        template = hints.get("recommended_template")
        if template:
            reasons.append(f"template_hint:{template}")
        if hints.get("sequence_based"):
            reasons.append("sequence_based:true")

    recent_seq = llm_input.get("recent_sequence") or []
    seq_text = _short_sequence(recent_seq)
    if seq_text:
        reasons.append(f"recent_sequence:{seq_text}")

    key_tokens = llm_input.get("key_event_tokens") or []
    if key_tokens:
        reasons.append("key_events:" + ",".join(key_tokens[:6]))

    if isinstance(pattern, dict):
        seq_patterns = pattern.get("sequence_patterns") or []
        if seq_patterns:
            seq = seq_patterns[0].get("sequence") or []
            support = seq_patterns[0].get("support")
            seq_text = _short_sequence(seq)
            if seq_text:
                reasons.append(f"pattern_sequence:{seq_text} (support={support})")
        trans = pattern.get("transition_patterns") or []
        if trans:
            first = trans[0]
            if first.get("from") and first.get("to"):
                reasons.append(f"pattern_transition:{first.get('from')}->{first.get('to')}")
        hourly = pattern.get("patterns") or []
        if hourly:
            top = hourly[0]
            if top.get("hour") and top.get("app"):
                reasons.append(f"hourly_pattern:{top.get('hour')}:{top.get('app')}")

    if tools:
        reasons.append("tools:" + ",".join(tools))
    return reasons[:8]


def _run_generator(config: str, llm_input: str, output: Path, profile: str) -> bool:
    cmd = [
        sys.executable,
        str(PROJECT_ROOT / "scripts" / "generate_n8n_workflow.py"),
        "--config",
        config,
        "--input",
        llm_input,
        "--output",
        str(output),
    ]
    if profile:
        cmd += ["--profile", profile]
    subprocess.run(cmd, check=False, cwd=PROJECT_ROOT)
    return output.exists()


def _run_template_candidate(config: str, llm_input: str, output: Path, profile: str) -> bool:
    env = dict(os.environ)
    api_key_env = "OPENAI_API_KEY"
    env[api_key_env] = ""
    cmd = [
        sys.executable,
        str(PROJECT_ROOT / "scripts" / "generate_n8n_workflow.py"),
        "--config",
        config,
        "--input",
        llm_input,
        "--output",
        str(output),
    ]
    if profile:
        cmd += ["--profile", profile]
    subprocess.run(cmd, check=False, env=env, cwd=PROJECT_ROOT)
    return output.exists()


def main() -> None:
    args = parse_args()
    out_dir = Path(args.output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    failure_summary = _summarize_failures(_load_quality_events(out_dir))

    feedback = _load_feedback(args.feedback)
    llm_input_path = args.input
    llm_input = _load_json(llm_input_path)
    pattern = _load_pattern(args.pattern, out_dir)

    candidates: list[dict] = []
    for idx in range(max(1, int(args.candidates))):
        out_path = out_dir / f"workflow_candidate_llm_{idx+1}.json"
        if _run_generator(args.config, llm_input_path, out_path, args.profile):
            payload = _load_json(str(out_path))
            fid = _fingerprint(payload) if isinstance(payload, dict) else f"llm_{idx+1}"
            tools = _tools_in_workflow(payload) if isinstance(payload, dict) else []
            base_score, notes = _score(str(out_path), args.profile, args.score_config, llm_input_path)
            adjust, reasons, pinned, excluded = _feedback_adjustment(fid, tools, feedback)
            if excluded:
                continue
            candidates.append(
                {
                    "id": fid,
                    "source": "llm",
                    "path": str(out_path),
                    "tools": tools,
                    "base_score": base_score,
                    "final_score": base_score + (1000 if pinned else 0) + adjust,
                    "notes": notes,
                    "adjustments": reasons,
                    "pinned": pinned,
                    "rationale": _build_rationale(llm_input, pattern, tools),
                }
            )

    if args.include_template:
        out_path = out_dir / "workflow_candidate_template.json"
        if _run_template_candidate(args.config, llm_input_path, out_path, args.profile):
            payload = _load_json(str(out_path))
            fid = _fingerprint(payload) if isinstance(payload, dict) else "template"
            tools = _tools_in_workflow(payload) if isinstance(payload, dict) else []
            base_score, notes = _score(str(out_path), args.profile, args.score_config, llm_input_path)
            adjust, reasons, pinned, excluded = _feedback_adjustment(fid, tools, feedback)
            if not excluded:
                candidates.append(
                    {
                        "id": fid,
                        "source": "template",
                        "path": str(out_path),
                        "tools": tools,
                        "base_score": base_score,
                        "final_score": base_score + (1000 if pinned else 0) + adjust,
                        "notes": notes,
                        "adjustments": reasons,
                        "pinned": pinned,
                        "rationale": _build_rationale(llm_input, pattern, tools),
                    }
                )

    if not candidates:
        print("no_candidates")
        return

    candidates = sorted(
        candidates, key=lambda c: (c["final_score"], c["base_score"]), reverse=True
    )

    top3 = candidates[:3]
    for item in top3:
        item["failure_summary"] = failure_summary
    _write_json(out_dir / "workflow_candidates.json", candidates)
    _write_json(out_dir / "workflow_recommendations.json", top3)

    md_lines = ["# Workflow Recommendations", ""]
    for idx, item in enumerate(top3, start=1):
        md_lines.append(f"## {idx}. {item['id']} (score {item['final_score']})")
        md_lines.append(f"- source: {item['source']}")
        md_lines.append(f"- tools: {', '.join(item['tools']) if item['tools'] else 'none'}")
        if item.get("pinned"):
            md_lines.append("- pinned: true (approved)")
        if item.get("notes"):
            md_lines.append(f"- notes: {', '.join(item['notes'][:6])}")
        if item.get("adjustments"):
            md_lines.append(f"- adjustments: {', '.join(item['adjustments'])}")
        if item.get("rationale"):
            md_lines.append(f"- rationale: {', '.join(item['rationale'])}")
        md_lines.append("")
    (out_dir / "workflow_recommendations.md").write_text("\n".join(md_lines), encoding="utf-8")

    best = candidates[0]
    save_best = Path(args.save_best)
    save_best.parent.mkdir(parents=True, exist_ok=True)
    if Path(best["path"]) != save_best:
        save_best.write_text(Path(best["path"]).read_text(encoding="utf-8"), encoding="utf-8")
    print(f"best_workflow_saved={save_best}")


if __name__ == "__main__":
    main()
