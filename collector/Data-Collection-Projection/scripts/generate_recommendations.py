from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate activity recommendations")
    parser.add_argument("--input", default="llm_input.json", help="llm_input path")
    parser.add_argument(
        "--config", default="", help="optional config to enable LLM"
    )
    parser.add_argument(
        "--output-md",
        default="",
        help="optional markdown output path",
    )
    parser.add_argument(
        "--output-json",
        default="",
        help="optional json output path",
    )
    return parser.parse_args()


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def main() -> None:
    args = parse_args()
    data = _load_json(args.input)
    config = load_config(args.config) if args.config else None
    min_conf = None
    if config and getattr(config, "automation", None):
        min_conf = config.automation.min_confidence
    payload = _build_recommendations(data, min_conf)

    if config and config.llm.enabled and config.llm.endpoint:
        llm_payload = _call_llm(config.llm, data, payload)
        if llm_payload:
            payload = llm_payload

    output_lines = _to_markdown(payload)
    output_md = "\n".join(output_lines)

    if args.output_md:
        out_path = Path(args.output_md)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(output_md, encoding="utf-8")
        print(f"recommendations_saved={out_path}")
    else:
        print(output_md)

    if args.output_json:
        out_path = Path(args.output_json)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"recommendations_json_saved={out_path}")


def _build_recommendations(data: dict, min_conf: float | None) -> dict:
    recs: List[Dict[str, Any]] = []
    for item in data.get("hourly_patterns", []):
        hour = item.get("hour")
        app = item.get("app")
        minutes = item.get("minutes")
        days = item.get("days")
        confidence = item.get("confidence")
        if not hour or not app:
            continue
        context = f"{hour} (최근 {days}일, 약 {minutes}분)" if days and minutes is not None else hour
        recs.append(
            {
                "text": f"{context}에 보통 {app}을(를) 사용합니다. 해당 시간에 자동으로 {app} 관련 작업을 준비하세요.",
                "confidence": confidence if confidence is not None else 0.5,
                "action": {"type": "open_app", "target": app},
                "reason": "hourly_pattern",
            }
        )

    top_apps = data.get("top_apps", [])
    if top_apps:
        top_app = top_apps[0].get("app")
        top_minutes = top_apps[0].get("minutes")
        if top_app:
            recs.append(
                {
                    "text": f"오늘 가장 많이 사용한 앱은 {top_app}입니다{f' ({top_minutes}분)' if top_minutes is not None else ''}. 집중 블록을 예약하거나 자동으로 관련 파일을 열어보세요.",
                    "confidence": 0.7,
                    "action": {"type": "open_app", "target": top_app},
                    "reason": "top_app",
                }
            )
            # Add optional create_file action suggestion for common apps.
            app_key = str(top_app).lower()
            if any(key in app_key for key in ["notion", "code", "vscode", "chrome"]):
                filename = f"{data.get('date_local','today')}_{app_key}_note.md"
                recs.append(
                    {
                        "text": f"{top_app} 작업 요약을 남길 파일을 미리 만들어두세요.",
                        "confidence": 0.55,
                        "action": {
                            "type": "create_file",
                            "target": str(Path("logs/run4/auto") / filename),
                            "app": top_app,
                        },
                        "reason": "create_file_suggestion",
                    }
                )

    # Weekday-specific pattern
    weekday_patterns = data.get("weekday_patterns") or {}
    try:
        today_key = datetime.now().strftime("%a")
    except Exception:
        today_key = ""
    if today_key and today_key in weekday_patterns:
        items = weekday_patterns.get(today_key) or []
        if items:
            item = items[0]
            hour = item.get("hour")
            app = item.get("app")
            confidence = item.get("confidence", 0.5)
            if hour and app:
                recs.append(
                    {
                        "text": f"{today_key} {hour}에는 보통 {app}을(를) 사용합니다. 해당 시간에 {app} 작업을 미리 준비하세요.",
                        "confidence": confidence,
                        "action": {"type": "open_app", "target": app},
                        "reason": "weekday_pattern",
                    }
                )

    # Sequence-based pattern
    sequences = data.get("sequence_patterns") or []
    if sequences:
        seq = sequences[0]
        seq_list = seq.get("sequence") or []
        if seq_list:
            seq_text = " → ".join(seq_list)
            confidence = seq.get("confidence", 0.5)
            recs.append(
                {
                    "text": f"최근 반복 흐름: {seq_text}. 흐름 시작 앱을 바로 열어두세요.",
                    "confidence": confidence,
                    "action": {"type": "open_app", "target": seq_list[0]},
                    "reason": "sequence_pattern",
                }
            )
            first_app = str(seq_list[0]).lower()
            if any(key in first_app for key in ["notion", "code", "vscode", "chrome"]):
                filename = f"{data.get('date_local','today')}_{first_app}_flow.md"
                recs.append(
                    {
                        "text": f"{seq_list[0]} 흐름을 위한 요약 파일을 미리 생성해 두세요.",
                        "confidence": confidence,
                        "action": {
                            "type": "create_file",
                            "target": str(Path("logs/run4/auto") / filename),
                            "app": seq_list[0],
                        },
                        "reason": "sequence_create_file",
                    }
                )

    # Context snapshot from top titles
    top_titles = data.get("top_titles") or []
    if top_titles:
        top_title = top_titles[0]
        title_hint = top_title.get("title_hint")
        app = top_title.get("app")
        minutes = top_title.get("minutes")
        if title_hint and app:
            recs.append(
                {
                    "text": f"최근 {app}에서 '{title_hint}' 작업이 자주 보였습니다{f' ({minutes}분)' if minutes is not None else ''}. 이어서 진행할까요?",
                    "confidence": 0.55,
                    "action": {"type": "open_app", "target": app},
                    "reason": "context_snapshot",
                }
            )

    if not recs:
        recs.append(
            {
                "text": "현재는 추천 패턴이 충분하지 않습니다. 데이터를 더 수집해 보세요.",
                "confidence": 0.0,
                "action": {"type": "none", "target": ""},
                "reason": "insufficient_data",
            }
        )

    return {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "items": recs,
        "source": "heuristic",
    }


def _to_markdown(payload: dict) -> list[str]:
    lines = [
        "# Activity Recommendations",
        "",
        f"- generated_at: {payload.get('generated_at')}",
        f"- source: {payload.get('source')}",
        "",
    ]
    for rec in payload.get("items", []):
        lines.append(f"- {rec.get('text')}")
    return lines


def _call_llm(llm_config, llm_input: dict, fallback: dict) -> dict:
    api_key = ""
    if llm_config.api_key_env:
        api_key = os.getenv(llm_config.api_key_env, "")

    schema_path = PROJECT_ROOT / "schemas" / "recommendations.schema.json"
    schema_text = ""
    if schema_path.exists():
        schema_text = schema_path.read_text(encoding="utf-8")

    prompt = {
        "task": "Generate activity recommendations and optional automation actions based on llm_input.",
        "constraints": [
            "Return JSON only.",
            "Follow the provided JSON schema exactly.",
            "Allowed action types: open_app, open_url, open_path, create_file, none.",
            "Do not include sensitive raw content.",
            "Write recommendation text in natural, friendly Korean.",
            "Keep each sentence concise and actionable.",
        ],
        "style_guide": {
            "language": "ko",
            "tone": "friendly, concise, helpful",
            "format": "short bullet-like sentences",
        },
        "schema": schema_text,
        "llm_input": llm_input,
        "fallback": fallback,
    }

    body = {
        "model": llm_config.model,
        "input": prompt,
        "max_tokens": llm_config.max_tokens,
    }
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["Authorization"] = f"Bearer {api_key}"

    try:
        req = urllib.request.Request(
            llm_config.endpoint,
            data=json.dumps(body, ensure_ascii=False).encode("utf-8"),
            headers=headers,
        )
        with urllib.request.urlopen(req, timeout=llm_config.timeout_sec) as resp:
            raw = resp.read().decode("utf-8", errors="ignore")
    except Exception:
        return {}

    try:
        parsed = json.loads(raw)
    except Exception:
        return {}

    if isinstance(parsed, dict) and "items" in parsed:
        parsed["generated_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        parsed["source"] = "llm"
        return parsed

    # Handle common LLM response wrappers (choices/message/content).
    content = ""
    if isinstance(parsed, dict):
        choices = parsed.get("choices")
        if isinstance(choices, list) and choices:
            message = choices[0].get("message") if isinstance(choices[0], dict) else None
            if message and isinstance(message, dict):
                content = str(message.get("content") or "")
            else:
                content = str(choices[0].get("text") or "")
    if content:
        try:
            parsed_content = json.loads(content)
            if isinstance(parsed_content, dict) and "items" in parsed_content:
                parsed_content["generated_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
                parsed_content["source"] = "llm"
                return parsed_content
        except Exception:
            return {}
    return {}


if __name__ == "__main__":
    main()
