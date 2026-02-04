from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate n8n workflow JSON")
    parser.add_argument("--input", default="llm_input.json", help="llm_input path")
    parser.add_argument("--config", default="", help="optional config to enable LLM")
    parser.add_argument("--output", default="n8n_workflow.json", help="output path")
    parser.add_argument("--name", default="DCP Auto Workflow", help="workflow name")
    parser.add_argument(
        "--webhook-path",
        default="dcp-workflow",
        help="default webhook path hint for LLM",
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


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _fallback_workflow(name: str, webhook_path: str) -> dict:
    return {
        "name": name,
        "active": False,
        "settings": {},
        "nodes": [
            {
                "id": "webhook_trigger",
                "name": "Webhook",
                "type": "n8n-nodes-base.webhook",
                "typeVersion": 1,
                "position": [300, 300],
                "parameters": {
                    "httpMethod": "POST",
                    "path": webhook_path,
                    "responseMode": "onReceived",
                },
            }
        ],
        "connections": {},
        "meta": {
            "generated_at": _now_utc(),
            "source": "fallback",
            "note": "LLM output unavailable or invalid. Replace this skeleton in n8n.",
        },
    }


def _validate(payload: dict) -> bool:
    if not isinstance(payload, dict):
        return False
    if not payload.get("name") or not isinstance(payload.get("name"), str):
        return False
    nodes = payload.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        return False
    connections = payload.get("connections")
    if connections is None or not isinstance(connections, dict):
        return False
    return True


def _call_llm(llm_config, llm_input: dict, fallback: dict, name: str, webhook_path: str) -> dict:
    api_key = ""
    if llm_config.api_key_env:
        api_key = os.getenv(llm_config.api_key_env, "")

    schema_path = PROJECT_ROOT / "schemas" / "n8n_workflow.schema.json"
    schema_text = ""
    if schema_path.exists():
        schema_text = schema_path.read_text(encoding="utf-8")

    prompt = {
        "task": "Generate a valid n8n workflow JSON from llm_input activity patterns.",
        "constraints": [
            "Return JSON only.",
            "Follow the provided JSON schema exactly.",
            "Prefer a single trigger node and 2-5 action/logic nodes.",
            "Use common built-in n8n nodes only.",
            "Do not include sensitive raw content.",
        ],
        "style_guide": {
            "language": "en",
            "tone": "concise",
        },
        "n8n_hints": {
            "workflow_name": name,
            "default_webhook_path": webhook_path,
            "recommended_nodes": [
                "n8n-nodes-base.webhook",
                "n8n-nodes-base.set",
                "n8n-nodes-base.if",
                "n8n-nodes-base.httpRequest",
                "n8n-nodes-base.cron",
            ],
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

    if _validate(parsed):
        parsed.setdefault("meta", {})
        parsed["meta"]["generated_at"] = _now_utc()
        parsed["meta"]["source"] = "llm"
        return parsed

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
            if _validate(parsed_content):
                parsed_content.setdefault("meta", {})
                parsed_content["meta"]["generated_at"] = _now_utc()
                parsed_content["meta"]["source"] = "llm"
                return parsed_content
        except Exception:
            return {}
    return {}


def main() -> None:
    args = parse_args()
    llm_input = _load_json(args.input)
    fallback = _fallback_workflow(args.name, args.webhook_path)
    payload = fallback

    config = load_config(args.config) if args.config else None
    if config and config.llm.enabled and config.llm.endpoint:
        llm_payload = _call_llm(config.llm, llm_input, fallback, args.name, args.webhook_path)
        if llm_payload:
            payload = llm_payload

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"n8n_workflow_saved={out_path}")


if __name__ == "__main__":
    main()
