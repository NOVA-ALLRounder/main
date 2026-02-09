from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


TEMPLATES = [
    {
        "id": "daily_summary_to_notion",
        "description": "Create a daily summary page in Notion from recent activity.",
        "required_nodes": ["webhook|cron", "notion"],
        "optional_nodes": ["if", "set"],
        "notes": "Use profile.ids.notion_database_id as databaseId and include key_events/top_apps.",
    },
    {
        "id": "focus_report_to_slack",
        "description": "Post a focus report to Slack.",
        "required_nodes": ["webhook|cron", "slack"],
        "optional_nodes": ["if", "set"],
        "notes": "Use profile.ids.slack_channel; message includes top_apps or recent_sequence.",
    },
    {
        "id": "followup_email_draft",
        "description": "Draft a follow-up email in Gmail.",
        "required_nodes": ["webhook|cron", "gmail"],
        "optional_nodes": ["if", "set"],
        "notes": "Use profile.ids.gmail_to; subject/body reference key_events.",
    },
    {
        "id": "file_save_followup",
        "description": "Create a Notion task when file save/export events spike.",
        "required_nodes": ["webhook|cron", "notion"],
        "optional_nodes": ["if", "set"],
        "notes": "Trigger when key_event_tokens include save/export/upload.",
    },
    {
        "id": "sequence_triggered_notification",
        "description": "Trigger a notification when a recent app sequence pattern is detected.",
        "required_nodes": ["webhook|cron", "if", "slack|notion|gmail"],
        "optional_nodes": ["set"],
        "notes": "Use recent_sequence or app_transitions to gate actions.",
    },
]


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
    parser.add_argument("--profile", default="", help="personalization profile json")
    return parser.parse_args()


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        return {}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


def _load_profile(path: str) -> dict:
    if path:
        return _load_json(path)
    try:
        default_path = Path(__file__).resolve().parents[1] / "configs" / "personalization_demo.json"
        return _load_json(str(default_path))
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


def _is_simple_context(llm_input: dict) -> bool:
    if not isinstance(llm_input, dict):
        return False
    top_apps = llm_input.get("top_apps") or []
    key_events = llm_input.get("key_events") or {}
    recent_events = llm_input.get("recent_events") or []
    quality = llm_input.get("quality") or {}
    total_events = quality.get("total_events", 0)
    return (
        len(top_apps) <= 2
        and len(key_events) <= 3
        and len(recent_events) <= 6
        and (total_events <= 30 if isinstance(total_events, int) else True)
    )


def _apply_profile_ids(payload: dict, profile: dict) -> None:
    if not isinstance(payload, dict) or not isinstance(profile, dict):
        return
    ids = profile.get("ids") or {}
    if not isinstance(ids, dict) or not ids:
        return
    nodes = payload.get("nodes")
    if not isinstance(nodes, list):
        return
    key_map = {
        "notion_database_id": "databaseId",
        "slack_channel": "channel",
        "gmail_to": "toEmail",
    }
    for node in nodes:
        if not isinstance(node, dict):
            continue
        params = node.get("parameters")
        if not isinstance(params, dict):
            continue
        for key, expected in ids.items():
            param_key = key_map.get(key, key)
            current = params.get(param_key)
            if not isinstance(expected, str) or not expected:
                continue
            if isinstance(current, str) and (
                "your_" in current.lower()
                or "placeholder" in current.lower()
                or current.strip() in {"", "REPLACE_ME"}
                or "profile.ids" in current.lower()
            ):
                params[param_key] = expected


def _summarize_llm_input(llm_input: dict) -> dict:
    if not isinstance(llm_input, dict):
        return {}
    top_apps = [item.get("app") for item in (llm_input.get("top_apps") or []) if item.get("app")]
    transitions = llm_input.get("app_transitions") or []
    key_tokens = llm_input.get("key_event_tokens") or []
    recent_sequence = llm_input.get("recent_sequence") or []
    return {
        "top_apps": top_apps[:5],
        "key_event_tokens": key_tokens[:12],
        "recent_sequence": recent_sequence[:12],
        "top_transitions": transitions[:5],
    }


def _preferred_template(llm_input: dict, profile: dict) -> dict:
    hints = (llm_input.get("workflow_hints") or {}) if isinstance(llm_input, dict) else {}
    recommended = hints.get("recommended_template") if isinstance(hints, dict) else ""
    if recommended:
        return {"id": recommended, "reason": "workflow_hints"}

    intent_summary = (llm_input.get("intent_summary") or {}) if isinstance(llm_input, dict) else {}
    primary = intent_summary.get("primary_intent")
    intent_to_template = {
        "daily_summary": "daily_summary_to_notion",
        "team_update": "focus_report_to_slack",
        "followup_email": "followup_email_draft",
        "file_backup": "file_save_followup",
        "focus_recap": "focus_report_to_slack",
    }
    if primary in intent_to_template:
        return {"id": intent_to_template[primary], "reason": "intent_summary"}

    recent_sequence = llm_input.get("recent_sequence") if isinstance(llm_input, dict) else []
    if isinstance(recent_sequence, list) and len(recent_sequence) >= 3:
        return {"id": "sequence_triggered_notification", "reason": "recent_sequence"}

    safety = profile.get("safety") if isinstance(profile, dict) else {}
    allow_actions = [a.lower() for a in (safety.get("allow_actions") or [])]
    if "slack" in allow_actions:
        return {"id": "focus_report_to_slack", "reason": "allow_actions"}
    if "notion" in allow_actions:
        return {"id": "daily_summary_to_notion", "reason": "allow_actions"}
    if "gmail" in allow_actions:
        return {"id": "followup_email_draft", "reason": "allow_actions"}
    return {}


def _load_env_key(var_name: str) -> None:
    if os.getenv(var_name):
        return
    # Best-effort: look for .env in repo root and load OPENAI_API_KEY
    try:
        for parent in Path(__file__).resolve().parents:
            env_path = parent / ".env"
            if not env_path.exists():
                continue
            for line in env_path.read_text(encoding="utf-8").splitlines():
                if not line or line.strip().startswith("#"):
                    continue
                if "=" not in line:
                    continue
                key, value = line.split("=", 1)
                key = key.strip()
                if key == var_name:
                    os.environ[key] = value.strip()
                    return
    except Exception:
        return


def _call_llm(
    llm_config,
    llm_input: dict,
    fallback: dict,
    name: str,
    webhook_path: str,
    profile: dict,
) -> dict:
    api_key = ""
    if llm_config.api_key_env:
        _load_env_key(llm_config.api_key_env)
        api_key = os.getenv(llm_config.api_key_env, "")

    schema_path = PROJECT_ROOT / "schemas" / "n8n_workflow.schema.json"
    schema_text = ""
    if schema_path.exists():
        schema_text = schema_path.read_text(encoding="utf-8")

    simple_mode = _is_simple_context(llm_input)
    recommended_template = _preferred_template(llm_input, profile)
    prompt = {
        "task": "Generate a valid n8n workflow JSON from llm_input activity patterns.",
        "constraints": [
            "Return JSON only.",
            "Follow the provided JSON schema exactly.",
            "Prefer a single trigger node and 2-5 action/logic nodes.",
            "Use common built-in n8n nodes only.",
            "Do not include sensitive raw content.",
            "Avoid placeholder URLs like example.com.",
            "If profile.tools is provided, include at least one node that matches those tools (e.g., Slack/Notion/Gmail).",
            "Include at least one IF node that uses key_events from llm_input to gate actions.",
            "If profile.ids is provided, use those identifiers instead of placeholders (e.g., databaseId, channel, toEmail).",
            "Include a time-window condition using profile.preferences.working_hours (e.g., only during working hours).",
            "Include both true and false branches from the IF node (e.g., notify vs. defer).",
            "Do not use generic httpRequest nodes for Notion/Slack/Gmail if native nodes exist.",
            "IF node must use true/false outputs (not main) for branching.",
            "If profile.safety.allow_actions is set, only use those action tools.",
            "If profile.safety.require_approval is true, include an IF gate before any action nodes.",
            "If llm_input indicates a simple context, generate a minimal workflow with 1 trigger + 1 action + optional IF.",
            "If llm_input.recent_sequence has 3+ apps, use recent_sequence/app_transitions to gate actions (sequence-based automation).",
        ],
        "style_guide": {"language": "en", "tone": "concise"},
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
        "llm_summary": _summarize_llm_input(llm_input),
        "templates": TEMPLATES,
        "recommended_template": recommended_template,
        "profile": profile,
        "fallback": fallback,
        "context_hints": {"simple_mode": simple_mode},
    }

    messages = [
        {"role": "system", "content": "You are a strict JSON generator."},
        {"role": "user", "content": json.dumps(prompt, ensure_ascii=False)},
    ]

    body = {
        "model": llm_config.model,
        "messages": messages,
        "max_tokens": llm_config.max_tokens,
        "temperature": 0.2,
        "response_format": {"type": "json_object"},
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
    except urllib.error.HTTPError as exc:
        try:
            raw = exc.read().decode("utf-8", errors="ignore")
            print(f"llm_http_error status={exc.code} body={raw[:2000]}")
        except Exception:
            print(f"llm_http_error status={exc.code}")
        return {}
    except Exception as exc:
        print(f"llm_request_failed: {exc}")
        return {}

    try:
        parsed = json.loads(raw)
    except Exception:
        print("llm_invalid_json_response")
        return {}

    if _validate(parsed):
        parsed.setdefault("meta", {})
        parsed["meta"]["generated_at"] = _now_utc()
        parsed["meta"]["source"] = "llm"
        _apply_profile_ids(parsed, profile)
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
        elif "output" in parsed:
            # Responses API style
            content = json.dumps(parsed.get("output"), ensure_ascii=False)
    if content:
        try:
            parsed_content = json.loads(content)
            if _validate(parsed_content):
                parsed_content.setdefault("meta", {})
                parsed_content["meta"]["generated_at"] = _now_utc()
                parsed_content["meta"]["source"] = "llm"
                _apply_profile_ids(parsed_content, profile)
                return parsed_content
        except Exception:
            return {}
    return {}


def main() -> None:
    args = parse_args()
    llm_input = _load_json(args.input)
    profile = _load_profile(args.profile)
    fallback = _fallback_workflow(args.name, args.webhook_path)
    payload = fallback

    config = load_config(args.config) if args.config else None
    if config and config.llm.enabled and config.llm.endpoint:
        llm_payload = _call_llm(
            config.llm, llm_input, fallback, args.name, args.webhook_path, profile
        )
        if llm_payload:
            payload = llm_payload
            _ensure_time_window_gate(payload, profile)
            _ensure_sequence_usage(payload, llm_input)
            _inject_sequence_context(payload, llm_input)
    else:
        _ensure_time_window_gate(payload, profile)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"n8n_workflow_saved={out_path}")


def _apply_time_window(payload: dict, profile: dict) -> bool:
    if not isinstance(payload, dict):
        return False
    working_hours = (
        (profile.get("preferences") or {}).get("working_hours") if isinstance(profile, dict) else None
    )
    if not isinstance(working_hours, str) or "-" not in working_hours:
        return False
    try:
        start_s, end_s = [part.strip() for part in working_hours.split("-", 1)]
        start_hour = int(start_s.split(":")[0])
        end_hour = int(end_s.split(":")[0])
    except Exception:
        return False

    nodes = payload.get("nodes")
    if not isinstance(nodes, list):
        return False

    for node in nodes:
        if not isinstance(node, dict):
            continue
        if "if" not in str(node.get("type", "")).lower():
            continue
        params = node.setdefault("parameters", {})
        conditions = params.setdefault("conditions", {})
        # If time window already present, skip.
        if "working_hours" in json.dumps(conditions, ensure_ascii=False):
            return True
        if "$now" in json.dumps(conditions, ensure_ascii=False):
            return True
        number_conditions = conditions.setdefault("number", [])
        if not isinstance(number_conditions, list):
            number_conditions = []
            conditions["number"] = number_conditions
        number_conditions.append(
            {"value1": "={{$now.hour}}", "operation": "largerEqual", "value2": start_hour}
        )
        number_conditions.append(
            {"value1": "={{$now.hour}}", "operation": "smaller", "value2": end_hour}
        )
        return True
    return False


def _ensure_time_window_gate(payload: dict, profile: dict) -> None:
    if _apply_time_window(payload, profile):
        return
    working_hours = (
        (profile.get("preferences") or {}).get("working_hours") if isinstance(profile, dict) else None
    )
    if not isinstance(working_hours, str) or "-" not in working_hours:
        return
    try:
        start_s, end_s = [part.strip() for part in working_hours.split("-", 1)]
        start_hour = int(start_s.split(":")[0])
        end_hour = int(end_s.split(":")[0])
    except Exception:
        return

    nodes = payload.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        return

    trigger = None
    for node in nodes:
        if not isinstance(node, dict):
            continue
        if any(t in str(node.get("type", "")).lower() for t in ["webhook", "cron"]):
            trigger = node
            break
    if not trigger:
        return

    if_id = "if_time_window"
    if_node = {
        "id": if_id,
        "name": "Working Hours",
        "type": "n8n-nodes-base.if",
        "typeVersion": 1,
        "position": [
            (trigger.get("position") or [300, 300])[0] + 200,
            (trigger.get("position") or [300, 300])[1],
        ],
        "parameters": {
            "conditions": {
                "number": [
                    {"value1": "={{$now.hour}}", "operation": "largerEqual", "value2": start_hour},
                    {"value1": "={{$now.hour}}", "operation": "smaller", "value2": end_hour},
                ]
            }
        },
    }
    nodes.append(if_node)

    connections = payload.setdefault("connections", {})
    if not isinstance(connections, dict):
        return

    trigger_key = None
    if trigger.get("id") in connections:
        trigger_key = trigger.get("id")
    elif trigger.get("name") in connections:
        trigger_key = trigger.get("name")
    else:
        trigger_key = trigger.get("id") or trigger.get("name")

    downstream_links = []
    if trigger_key and trigger_key in connections:
        existing = connections.get(trigger_key, {})
        if isinstance(existing, dict):
            main = existing.get("main")
            if isinstance(main, list) and main:
                downstream_links = main[0] if isinstance(main[0], list) else []

    connections[trigger_key] = {
        "main": [[{"node": if_id, "type": "main", "index": 0}]]
    }
    connections[if_id] = {"main": [downstream_links, []]}


def _expects_sequence(llm_input: dict) -> bool:
    if not isinstance(llm_input, dict):
        return False
    recent_sequence = llm_input.get("recent_sequence") or []
    transitions = llm_input.get("app_transitions") or []
    return (isinstance(recent_sequence, list) and len(recent_sequence) >= 3) or bool(transitions)


def _has_sequence_usage(payload: dict) -> bool:
    try:
        blob = json.dumps(payload.get("nodes", []), ensure_ascii=False).lower()
    except Exception:
        return False
    return "recent_sequence" in blob or "app_transitions" in blob or "sequence_signature" in blob


def _ensure_sequence_usage(payload: dict, llm_input: dict) -> None:
    if not _expects_sequence(llm_input):
        return
    if _has_sequence_usage(payload):
        return
    nodes = payload.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        return
    seq_value = "={{$json[\"sequence_signature\"]}}"
    if not llm_input.get("sequence_signature"):
        seq_value = "={{$json[\"recent_sequence\"]}}"

    # Find an IF node to inject sequence condition.
    for node in nodes:
        if not isinstance(node, dict):
            continue
        if "if" not in str(node.get("type", "")).lower():
            continue
        params = node.setdefault("parameters", {})
        conditions = params.setdefault("conditions", {})
        string_conditions = conditions.setdefault("string", [])
        if not isinstance(string_conditions, list):
            string_conditions = []
            conditions["string"] = string_conditions
        string_conditions.append(
            {"value1": seq_value, "operation": "notEmpty"}
        )
        return


def _sequence_expr(llm_input: dict) -> str:
    if llm_input.get("sequence_signature"):
        return "={{$json[\"sequence_signature\"]}}"
    return "={{$json[\"recent_sequence\"]}}"


def _inject_sequence_context(payload: dict, llm_input: dict) -> None:
    if not _expects_sequence(llm_input):
        return
    nodes = payload.get("nodes")
    if not isinstance(nodes, list):
        return
    seq_expr = _sequence_expr(llm_input)

    for node in nodes:
        if not isinstance(node, dict):
            continue
        node_type = str(node.get("type", "")).lower()
        params = node.get("parameters")
        if not isinstance(params, dict):
            continue
        # Slack message
        if "slack" in node_type:
            text = params.get("text")
            if isinstance(text, str) and "sequence_signature" not in text:
                params["text"] = text.rstrip() + f"\nSequence: {seq_expr}"
        # Gmail message
        if "gmail" in node_type:
            text = params.get("text")
            if isinstance(text, str) and "sequence_signature" not in text:
                params["text"] = text.rstrip() + f"\nSequence: {seq_expr}"
        # Notion page content
        if "notion" in node_type:
            props = params.get("properties")
            if isinstance(props, dict):
                for _, value in props.items():
                    if not isinstance(value, dict):
                        continue
                    rich = value.get("rich_text")
                    if isinstance(rich, list) and rich:
                        item = rich[0]
                        if isinstance(item, dict):
                            text = item.get("text", {})
                            if isinstance(text, dict):
                                content = text.get("content")
                                if isinstance(content, str) and "sequence_signature" not in content:
                                    text["content"] = content.rstrip() + f"\nSequence: {seq_expr}"


if __name__ == "__main__":
    main()
