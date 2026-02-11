from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

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
    base = _load_json(path) if path else {}
    try:
        default_path = Path(__file__).resolve().parents[1] / "configs" / "personalization_demo.json"
        if not base:
            base = _load_json(str(default_path))
    except Exception:
        pass
    learned = _load_json(str(Path(__file__).resolve().parents[1] / "configs" / "profile_learned.json"))
    return _merge_profiles(base, learned)


def _merge_profiles(base: dict, learned: dict) -> dict:
    if not isinstance(base, dict) and not isinstance(learned, dict):
        return {}
    merged: dict = {}
    if isinstance(base, dict):
        merged = json.loads(json.dumps(base))
    if isinstance(learned, dict) and learned:
        learned_overlay: dict = {}
        if learned.get("tools"):
            learned_overlay["tools"] = learned.get("tools")
        if learned.get("preferences"):
            learned_overlay["preferences"] = learned.get("preferences")
        if learned.get("signals"):
            learned_overlay["signals"] = learned.get("signals")
        if learned.get("generated_at"):
            learned_overlay["learned_at"] = learned.get("generated_at")
        merged = _deep_update(merged, learned_overlay)
    return merged


def _deep_update(target: dict, source: dict) -> dict:
    for key, value in source.items():
        if isinstance(value, dict) and isinstance(target.get(key), dict):
            target[key] = _deep_update(target.get(key, {}), value)
        else:
            target[key] = value
    return target


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _append_quality_event(path: Optional[Path], payload: dict) -> None:
    if not path:
        return
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        payload.setdefault("ts", _now_utc())
        with path.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(payload, ensure_ascii=False) + "\n")
    except Exception:
        return


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
        "google_calendar_id": "calendar",
        "kakao_webhook_url": "url",
    }

    def _is_placeholder(value: Any) -> bool:
        if not isinstance(value, str):
            return True
        lowered = value.strip().lower()
        if not lowered:
            return True
        return any(
            token in lowered
            for token in ("your_", "placeholder", "replace_me", "profile.ids")
        )
    for node in nodes:
        if not isinstance(node, dict):
            continue
        node_type = str(node.get("type", "")).lower()
        params = node.get("parameters")
        if not isinstance(params, dict):
            continue
        allowed_keys: list[str] = []
        if "notion" in node_type:
            allowed_keys.append("notion_database_id")
        if "slack" in node_type:
            allowed_keys.append("slack_channel")
        if "gmail" in node_type:
            allowed_keys.append("gmail_to")
        if "googlecalendar" in node_type:
            allowed_keys.append("google_calendar_id")
        if "http" in node_type and "kakao" in str(node.get("name", "")).lower():
            allowed_keys.append("kakao_webhook_url")

        for key in allowed_keys:
            expected = ids.get(key)
            param_key = key_map.get(key, key)
            current = params.get(param_key)
            if not isinstance(expected, str) or not expected:
                continue
            if current is None or _is_placeholder(current):
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


def _allowed_action_tools(profile: dict) -> list[str]:
    if not isinstance(profile, dict):
        return []
    safety = profile.get("safety") or {}
    allow_actions = [str(a).lower() for a in (safety.get("allow_actions") or []) if str(a).strip()]
    tools = [str(t).lower() for t in (profile.get("tools") or []) if str(t).strip()]
    raw = allow_actions if allow_actions else tools
    allowed = {"slack", "notion", "gmail", "googlecalendar", "kakao"}
    normalized: list[str] = []
    for item in raw:
        norm = _normalize_tool_name(item)
        if norm and norm in allowed and norm not in normalized:
            normalized.append(norm)
    return normalized


def _normalize_tool_name(raw: str) -> str:
    text = str(raw or "").lower().strip()
    if not text:
        return ""
    if "kakao" in text:
        return "kakao"
    if "google" in text and "calendar" in text:
        return "googlecalendar"
    if "calendar" in text:
        return "googlecalendar"
    if "gmail" in text or text in {"email", "mail"}:
        return "gmail"
    if "notion" in text:
        return "notion"
    if "slack" in text:
        return "slack"
    return text


def _select_tool(preferred: list[str], allowed: list[str]) -> str:
    if not preferred:
        return allowed[0] if allowed else ""
    for item in preferred:
        if item in allowed:
            return item
    return allowed[0] if allowed else ""


def _unique_node_name(nodes: list[dict], base: str) -> str:
    existing = {n.get("name") for n in nodes if isinstance(n, dict)}
    if base not in existing:
        return base
    idx = 2
    while f"{base} {idx}" in existing:
        idx += 1
    return f"{base} {idx}"


def _node_matches_tool(node: dict, tool: str) -> bool:
    if not isinstance(node, dict):
        return False
    node_type = str(node.get("type", "")).lower()
    name = str(node.get("name", "")).lower()
    if tool == "notion":
        return "notion" in node_type
    if tool == "slack":
        return "slack" in node_type
    if tool == "gmail":
        return "gmail" in node_type
    if tool == "googlecalendar":
        return "googlecalendar" in node_type
    if tool == "kakao":
        return "http" in node_type and "kakao" in name
    return False


def _template_workflow(template_id: str, name: str, webhook_path: str, profile: dict, llm_input: dict) -> dict:
    allowed = _allowed_action_tools(profile)
    template_id = template_id or ""

    tool_choices = {
        "daily_summary_to_notion": ["notion", "slack"],
        "focus_report_to_slack": ["slack", "notion"],
        "followup_email_draft": ["gmail"],
        "file_save_followup": ["notion", "slack"],
        "sequence_triggered_notification": ["slack", "notion", "gmail"],
    }
    preferred = tool_choices.get(template_id, [])
    tool = _select_tool(preferred, allowed)

    nodes = []
    trigger = {
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
    nodes.append(trigger)

    if_node = {
        "id": "if_gate",
        "name": "IF",
        "type": "n8n-nodes-base.if",
        "typeVersion": 1,
        "position": [520, 300],
        "parameters": {"conditions": {"boolean": [{"value1": "={{true}}", "operation": "equal", "value2": True}]}},
    }
    nodes.append(if_node)

    action_node = None
    if tool == "notion":
        action_node = {
            "id": "notion_node",
            "name": "Create Notion Page",
            "type": "n8n-nodes-base.notion",
            "typeVersion": 1,
            "position": [760, 200],
            "parameters": {
                "operation": "create",
                "databaseId": "REPLACE_ME",
                "properties": {
                    "Title": {"title": [{"text": {"content": "Daily Summary"}}]},
                    "Summary": {
                        "rich_text": [{"text": {"content": "Summary of recent activity."}}]
                    },
                },
            },
        }
    elif tool == "slack":
        action_node = {
            "id": "slack_node",
            "name": "Send Slack Message",
            "type": "n8n-nodes-base.slack",
            "typeVersion": 1,
            "position": [760, 320],
            "parameters": {
                "channel": "REPLACE_ME",
                "text": "Focus report generated based on recent activity.",
            },
        }
    elif tool == "gmail":
        action_node = {
            "id": "gmail_node",
            "name": "Draft Follow-up Email",
            "type": "n8n-nodes-base.gmail",
            "typeVersion": 1,
            "position": [760, 320],
            "parameters": {
                "toEmail": "REPLACE_ME",
                "subject": "Follow-up",
                "text": "Draft follow-up based on recent activity.",
            },
        }

    connections = {
        "Webhook": {"main": [[{"node": "IF", "type": "main", "index": 0}]]},
        "IF": {"main": [[], []]},
    }

    if action_node:
        nodes.append(action_node)
        connections["IF"]["main"][0] = [{"node": action_node["name"], "type": "main", "index": 0}]

    payload = {
        "name": name,
        "active": False,
        "settings": {},
        "nodes": nodes,
        "connections": connections,
        "meta": {
            "generated_at": _now_utc(),
            "source": "template",
            "note": template_id or "template_fallback",
        },
    }
    _apply_profile_ids(payload, profile)
    return payload


def _preferred_tools(profile: dict) -> list[str]:
    preferred: list[str] = []
    raw = profile.get("preferred_actions") if isinstance(profile, dict) else []
    if isinstance(raw, list):
        for item in raw:
            if isinstance(item, dict):
                tool = _normalize_tool_name(item.get("tool", ""))
                if tool:
                    preferred.append(tool)
            elif isinstance(item, str):
                tool = _normalize_tool_name(item)
                if tool:
                    preferred.append(tool)
    if preferred:
        # preserve order, remove duplicates
        seen = set()
        ordered = []
        for tool in preferred:
            if tool in seen:
                continue
            seen.add(tool)
            ordered.append(tool)
        return ordered
    return _allowed_action_tools(profile)


def _make_action_node(tool: str, name: str, position: list[int], llm_input: dict) -> dict:
    seq_expr = _sequence_expr(llm_input)
    if tool == "notion":
        return {
            "id": name.lower().replace(" ", "_"),
            "name": name,
            "type": "n8n-nodes-base.notion",
            "typeVersion": 1,
            "position": position,
            "parameters": {
                "operation": "create",
                "databaseId": "REPLACE_ME",
                "properties": {
                    "Title": {"title": [{"text": {"content": "Daily Summary"}}]},
                    "Summary": {
                        "rich_text": [
                            {"text": {"content": f"Summary of recent activity.\nSequence: {seq_expr}"}}
                        ]
                    },
                },
            },
        }
    if tool == "slack":
        return {
            "id": name.lower().replace(" ", "_"),
            "name": name,
            "type": "n8n-nodes-base.slack",
            "typeVersion": 1,
            "position": position,
            "parameters": {
                "channel": "REPLACE_ME",
                "text": f"Focus report generated.\nSequence: {seq_expr}",
            },
        }
    if tool == "gmail":
        return {
            "id": name.lower().replace(" ", "_"),
            "name": name,
            "type": "n8n-nodes-base.gmail",
            "typeVersion": 1,
            "position": position,
            "parameters": {
                "toEmail": "REPLACE_ME",
                "subject": "Follow-up",
                "text": f"Draft follow-up based on recent activity.\nSequence: {seq_expr}",
            },
        }
    if tool == "googlecalendar":
        return {
            "id": name.lower().replace(" ", "_"),
            "name": name,
            "type": "n8n-nodes-base.googleCalendar",
            "typeVersion": 1,
            "position": position,
            "parameters": {
                "operation": "create",
                "calendar": "primary",
                "summary": "Focus block",
                "description": f"Auto-created from activity.\nSequence: {seq_expr}",
                "start": "={{$now}}",
                "end": "={{$now.add(30, 'minutes')}}",
            },
        }
    if tool == "kakao":
        return {
            "id": name.lower().replace(" ", "_"),
            "name": name,
            "type": "n8n-nodes-base.httpRequest",
            "typeVersion": 2,
            "position": position,
            "parameters": {
                "url": "={{$json[\"kakao_webhook_url\"]}}",
                "method": "POST",
                "jsonParameters": True,
                "bodyParametersJson": "{\"text\":\"Auto workflow notification\"}",
            },
        }
    return {}


def _connection_key(payload: dict, node: dict) -> str:
    connections = payload.get("connections") if isinstance(payload, dict) else {}
    name = node.get("name")
    node_id = node.get("id")
    if isinstance(connections, dict):
        if name and name in connections:
            return name
        if node_id and node_id in connections:
            return node_id
    return name or node_id or ""


def _ensure_branch(connections: dict, key: str) -> list[list[dict]]:
    if key not in connections or not isinstance(connections.get(key), dict):
        connections[key] = {"main": [[], []]}
    main = connections[key].get("main")
    if not isinstance(main, list):
        main = [[], []]
        connections[key]["main"] = main
    while len(main) < 2:
        main.append([])
    if not isinstance(main[0], list):
        main[0] = []
    if not isinstance(main[1], list):
        main[1] = []
    return main


def _ensure_action_fanout(payload: dict, profile: dict, llm_input: dict) -> None:
    nodes = payload.get("nodes")
    if not isinstance(nodes, list):
        return
    if_node = _get_primary_if_node(payload)
    if not if_node:
        return

    target_tools = _preferred_tools(profile)
    if not target_tools:
        return

    existing_tools = {
        tool for tool in target_tools if any(_node_matches_tool(n, tool) for n in nodes)
    }
    missing = [tool for tool in target_tools if tool not in existing_tools]
    if not missing:
        return

    connections = payload.setdefault("connections", {})
    if_key = _connection_key(payload, if_node)
    branches = _ensure_branch(connections, if_key)

    base_x, base_y = (if_node.get("position") or [520, 300])
    offset = 0
    for tool in missing:
        name = _unique_node_name(nodes, tool.capitalize())
        node = _make_action_node(tool, name, [base_x + 220, base_y - 160 + offset], llm_input)
        if not node:
            continue
        nodes.append(node)
        branches[0].append({"node": node["name"], "type": "main", "index": 0})
        offset += 120

    _apply_profile_ids(payload, profile)


def _ensure_approval_fallback(payload: dict, profile: dict, llm_input: dict) -> None:
    safety = profile.get("safety") if isinstance(profile, dict) else {}
    require_approval = bool(safety.get("require_approval"))
    if not require_approval:
        return
    if_node = _get_primary_if_node(payload)
    if not if_node:
        return

    connections = payload.setdefault("connections", {})
    if_key = _connection_key(payload, if_node)
    branches = _ensure_branch(connections, if_key)
    if branches[1]:
        return

    allowed = _allowed_action_tools(profile)
    preferred = ["slack", "gmail", "notion"]
    tool = next((t for t in preferred if t in allowed), "")
    if not tool:
        return

    base_x, base_y = (if_node.get("position") or [520, 300])
    name = _unique_node_name(nodes=payload.get("nodes", []), base=f"{tool.capitalize()} Approval")
    node = _make_action_node(tool, name, [base_x + 220, base_y + 160], llm_input)
    if not node:
        return
    # override message to indicate approval needed
    if "parameters" in node and isinstance(node["parameters"], dict):
        if tool == "slack":
            node["parameters"]["text"] = "Approval needed before automation can run."
        if tool == "gmail":
            node["parameters"]["subject"] = "Approval Needed"
            node["parameters"]["text"] = "Approval needed before automation can run."
        if tool == "notion":
            props = node["parameters"].get("properties") or {}
            title = props.get("Title")
            if isinstance(title, dict):
                title["title"] = [{"text": {"content": "Approval Needed"}}]
            summary = props.get("Summary")
            if isinstance(summary, dict):
                summary["rich_text"] = [{"text": {"content": "Approval required before automation."}}]

    payload["nodes"].append(node)
    branches[1].append({"node": node["name"], "type": "main", "index": 0})


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

    task_summary = (llm_input.get("task_summary") or {}) if isinstance(llm_input, dict) else {}
    task_primary = task_summary.get("primary_task")
    task_map = {
        "doc_write": "daily_summary_to_notion",
        "communication": "focus_report_to_slack",
        "email": "followup_email_draft",
        "task_management": "focus_report_to_slack",
        "coding": "focus_report_to_slack",
    }
    if task_primary in task_map:
        return {"id": task_map[task_primary], "reason": "task_summary"}

    time_context = (llm_input.get("time_context") or {}) if isinstance(llm_input, dict) else {}
    hour_local = time_context.get("hour_local")
    hourly_patterns = llm_input.get("hourly_patterns") if isinstance(llm_input, dict) else []
    if hour_local is not None and isinstance(hourly_patterns, list):
        for item in hourly_patterns:
            try:
                if int(item.get("hour", -1)) != int(hour_local):
                    continue
            except Exception:
                continue
            app = str(item.get("app", "")).lower()
            if "notion" in app:
                return {"id": "daily_summary_to_notion", "reason": "hourly_patterns"}
            if "slack" in app or "kakaotalk" in app:
                return {"id": "focus_report_to_slack", "reason": "hourly_patterns"}
            if "gmail" in app or "outlook" in app:
                return {"id": "followup_email_draft", "reason": "hourly_patterns"}

    weekday_patterns = llm_input.get("weekday_patterns") if isinstance(llm_input, dict) else {}
    weekday_local = time_context.get("weekday_local")
    if weekday_local and isinstance(weekday_patterns, dict):
        items = weekday_patterns.get(weekday_local) or []
        if items:
            app = str(items[0].get("app", "")).lower()
            if "notion" in app:
                return {"id": "daily_summary_to_notion", "reason": "weekday_patterns"}
            if "slack" in app or "kakaotalk" in app:
                return {"id": "focus_report_to_slack", "reason": "weekday_patterns"}
            if "gmail" in app or "outlook" in app:
                return {"id": "followup_email_draft", "reason": "weekday_patterns"}

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
    quality_log: Optional[Path],
) -> dict:
    api_key = ""
    if llm_config.api_key_env:
        _load_env_key(llm_config.api_key_env)
        api_key = os.getenv(llm_config.api_key_env, "")
    if not api_key and llm_config.api_key_env:
        _append_quality_event(
            quality_log,
            {"type": "llm_error", "reason": "missing_api_key", "env": llm_config.api_key_env},
        )
        print(f"llm_missing_api_key env={llm_config.api_key_env}")
        return {}

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
            _append_quality_event(
                quality_log,
                {"type": "llm_error", "reason": "http_error", "status": exc.code},
            )
        except Exception:
            print(f"llm_http_error status={exc.code}")
            _append_quality_event(
                quality_log,
                {"type": "llm_error", "reason": "http_error", "status": exc.code},
            )
        return {}
    except Exception as exc:
        print(f"llm_request_failed: {exc}")
        _append_quality_event(
            quality_log,
            {"type": "llm_error", "reason": "request_failed", "error": str(exc)},
        )
        return {}

    try:
        parsed = json.loads(raw)
    except Exception:
        print("llm_invalid_json_response")
        _append_quality_event(
            quality_log,
            {"type": "llm_error", "reason": "invalid_json_response"},
        )
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
            _append_quality_event(
                quality_log,
                {"type": "llm_error", "reason": "schema_invalid"},
            )
        except Exception:
            _append_quality_event(
                quality_log,
                {"type": "llm_error", "reason": "content_json_invalid"},
            )
            return {}
    _append_quality_event(
        quality_log,
        {"type": "llm_error", "reason": "schema_invalid_no_content"},
    )
    return {}


def main() -> None:
    args = parse_args()
    llm_input = _load_json(args.input)
    profile = _load_profile(args.profile)
    fallback = _fallback_workflow(args.name, args.webhook_path)
    payload = fallback
    out_path = Path(args.output)
    quality_log = out_path.parent / "quality_events.jsonl"

    config = load_config(args.config) if args.config else None
    if config and config.llm.enabled and config.llm.endpoint:
        llm_payload = _call_llm(
            config.llm, llm_input, fallback, args.name, args.webhook_path, profile, quality_log
        )
        if llm_payload and _allowed_payload(llm_payload, profile):
            payload = llm_payload
        else:
            _append_quality_event(
                quality_log,
                {"type": "llm_fallback", "reason": "disallowed_or_empty"},
            )
            template_choice = _preferred_template(llm_input, profile)
            if template_choice.get("id"):
                payload = _template_workflow(
                    template_choice.get("id", ""),
                    args.name,
                    args.webhook_path,
                    profile,
                    llm_input,
                )
            else:
                payload = fallback
    else:
        _append_quality_event(
            quality_log,
            {"type": "llm_fallback", "reason": "llm_disabled"},
        )
        template_choice = _preferred_template(llm_input, profile)
        if template_choice.get("id"):
            payload = _template_workflow(
                template_choice.get("id", ""),
                args.name,
                args.webhook_path,
                profile,
                llm_input,
            )
        else:
            payload = fallback

    _apply_required_gates(payload, llm_input, profile)
    _ensure_action_fanout(payload, profile, llm_input)
    _ensure_approval_fallback(payload, profile, llm_input)
    _inject_sequence_context(payload, llm_input)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"n8n_workflow_saved={out_path}")


def _allowed_payload(payload: dict, profile: dict) -> bool:
    if not isinstance(payload, dict):
        return False
    safety = profile.get("safety") if isinstance(profile, dict) else {}
    allow_actions = [_normalize_tool_name(a) for a in (safety.get("allow_actions") or [])]
    if not allow_actions:
        return True
    nodes = payload.get("nodes") or []
    for node in nodes:
        if not isinstance(node, dict):
            continue
        node_type = str(node.get("type", "")).lower()
        action_map = {
            "slack": "slack",
            "notion": "notion",
            "gmail": "gmail",
            "googlecalendar": "googlecalendar",
        }
        for token, tool in action_map.items():
            if token in node_type and tool not in allow_actions:
                return False
        if "http" in node_type and "kakao" in str(node.get("name", "")).lower():
            if "kakao" not in allow_actions:
                return False
    return True


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

    def _has_now_hour(conditions: dict) -> bool:
        number_conditions = conditions.get("number")
        if not isinstance(number_conditions, list):
            return False
        for cond in number_conditions:
            if not isinstance(cond, dict):
                continue
            value1 = str(cond.get("value1") or "")
            if "$now.hour" in value1:
                return True
        return False

    for node in nodes:
        if not isinstance(node, dict):
            continue
        if "if" not in str(node.get("type", "")).lower():
            continue
        params = node.setdefault("parameters", {})
        conditions = params.setdefault("conditions", {})
        # If time window already present, skip.
        if _has_now_hour(conditions):
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
    if_node = _get_primary_if_node(payload)
    if if_node:
        _ensure_time_window_condition(if_node, profile, {})


def _get_primary_if_node(payload: dict) -> Optional[dict]:
    nodes = payload.get("nodes")
    if not isinstance(nodes, list) or not nodes:
        return None
    for node in nodes:
        if not isinstance(node, dict):
            continue
        if "if" in str(node.get("type", "")).lower():
            return node

    trigger = None
    for node in nodes:
        if not isinstance(node, dict):
            continue
        if any(t in str(node.get("type", "")).lower() for t in ["webhook", "cron"]):
            trigger = node
            break
    if not trigger:
        return None

    if_id = "if_gate"
    if_node = {
        "id": if_id,
        "name": "IF",
        "type": "n8n-nodes-base.if",
        "typeVersion": 1,
        "position": [
            (trigger.get("position") or [300, 300])[0] + 200,
            (trigger.get("position") or [300, 300])[1],
        ],
        "parameters": {"conditions": {"boolean": [{"value1": "={{true}}", "operation": "equal", "value2": True}]}},
    }
    nodes.append(if_node)

    connections = payload.setdefault("connections", {})
    if not isinstance(connections, dict):
        return if_node

    trigger_key = None
    if trigger.get("name") in connections:
        trigger_key = trigger.get("name")
    elif trigger.get("id") in connections:
        trigger_key = trigger.get("id")
    else:
        trigger_key = trigger.get("name") or trigger.get("id")

    downstream_links = []
    if trigger_key and trigger_key in connections:
        existing = connections.get(trigger_key, {})
        if isinstance(existing, dict):
            main = existing.get("main")
            if isinstance(main, list) and main:
                downstream_links = main[0] if isinstance(main[0], list) else []

    connections[trigger_key] = {
        "main": [[{"node": if_node["name"], "type": "main", "index": 0}]]
    }
    connections[if_node["name"]] = {"main": [downstream_links, []]}
    return if_node


def _resolve_working_hours(profile: dict, llm_input: dict) -> tuple[int, int] | None:
    working_hours = (
        (profile.get("preferences") or {}).get("working_hours") if isinstance(profile, dict) else None
    )
    if isinstance(working_hours, str) and "-" in working_hours:
        try:
            start_s, end_s = [part.strip() for part in working_hours.split("-", 1)]
            start_hour = int(start_s.split(":")[0])
            end_hour = int(end_s.split(":")[0])
            return start_hour, end_hour
        except Exception:
            return None

    time_context = llm_input.get("time_context") if isinstance(llm_input, dict) else None
    if isinstance(time_context, dict):
        hours = time_context.get("active_hours") or []
        try:
            hours = [int(h) for h in hours]
        except Exception:
            hours = []
        if len(hours) >= 2:
            start_hour = min(hours)
            end_hour = max(hours) + 1
            return start_hour, end_hour
    return None


def _ensure_time_window_condition(if_node: dict, profile: dict, llm_input: dict) -> None:
    working = _resolve_working_hours(profile, llm_input)
    if not working:
        return
    start_hour, end_hour = working
    params = if_node.setdefault("parameters", {})
    conditions = params.setdefault("conditions", {})
    number_conditions = conditions.setdefault("number", [])
    if not isinstance(number_conditions, list):
        number_conditions = []
        conditions["number"] = number_conditions
    blob = json.dumps(number_conditions, ensure_ascii=False)
    if "$now.hour" in blob:
        return
    number_conditions.append(
        {"value1": "={{$now.hour}}", "operation": "largerEqual", "value2": start_hour}
    )
    number_conditions.append(
        {"value1": "={{$now.hour}}", "operation": "smaller", "value2": end_hour}
    )


def _ensure_sequence_condition(if_node: dict, llm_input: dict) -> None:
    if not _expects_sequence(llm_input):
        return
    params = if_node.setdefault("parameters", {})
    conditions = params.setdefault("conditions", {})
    string_conditions = conditions.setdefault("string", [])
    if not isinstance(string_conditions, list):
        string_conditions = []
        conditions["string"] = string_conditions
    blob = json.dumps(string_conditions, ensure_ascii=False).lower()
    if "sequence_signature" in blob or "recent_sequence" in blob or "app_transitions" in blob:
        return
    seq_value = "={{$json[\"sequence_signature\"]}}"
    string_conditions.append({"value1": seq_value, "operation": "notEmpty"})


def _ensure_frequency_condition(if_node: dict, llm_input: dict) -> None:
    if not isinstance(llm_input, dict):
        return
    key_events = llm_input.get("key_events") or {}
    params = if_node.setdefault("parameters", {})
    conditions = params.setdefault("conditions", {})
    number_conditions = conditions.setdefault("number", [])
    if not isinstance(number_conditions, list):
        number_conditions = []
        conditions["number"] = number_conditions
    blob = json.dumps(number_conditions, ensure_ascii=False).lower()
    if "key_events" in blob or "active_minutes" in blob or "total_events" in blob:
        return
    focus_count = key_events.get("os.app_focus_block") or key_events.get("os.foreground_changed")
    if isinstance(focus_count, (int, float)):
        number_conditions.append(
            {
                "value1": "={{$json[\"key_events\"][\"os.app_focus_block\"]}}",
                "operation": "largerEqual",
                "value2": 2,
            }
        )
        return
    quality = llm_input.get("quality") or {}
    active_minutes = quality.get("active_minutes")
    if isinstance(active_minutes, (int, float)):
        number_conditions.append(
            {
                "value1": "={{$json[\"quality\"][\"active_minutes\"]}}",
                "operation": "largerEqual",
                "value2": 3,
            }
        )


def _ensure_approval_condition(if_node: dict, profile: dict, llm_input: dict) -> None:
    safety = profile.get("safety") if isinstance(profile, dict) else {}
    require_approval = bool(safety.get("require_approval"))
    auto_approve = bool(llm_input.get("auto_approve")) if isinstance(llm_input, dict) else False
    if not require_approval and not auto_approve:
        return
    params = if_node.setdefault("parameters", {})
    conditions = params.setdefault("conditions", {})
    boolean_conditions = conditions.setdefault("boolean", [])
    if not isinstance(boolean_conditions, list):
        boolean_conditions = []
        conditions["boolean"] = boolean_conditions
    blob = json.dumps(boolean_conditions, ensure_ascii=False).lower()
    if "auto_approve" in blob or "approval" in blob:
        return
    boolean_conditions.append(
        {
            "value1": "={{$json[\"auto_approve\"] === true || $json[\"approval\"] === \"approved\"}}",
            "operation": "equal",
            "value2": True,
        }
    )


def _apply_required_gates(payload: dict, llm_input: dict, profile: dict) -> None:
    if_node = _get_primary_if_node(payload)
    if not if_node:
        return
    _ensure_time_window_condition(if_node, profile, llm_input)
    _ensure_sequence_condition(if_node, llm_input)
    _ensure_frequency_condition(if_node, llm_input)
    _ensure_approval_condition(if_node, profile, llm_input)


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
        # Google Calendar description
        if "googlecalendar" in node_type:
            desc = params.get("description")
            if isinstance(desc, str) and "sequence_signature" not in desc:
                params["description"] = desc.rstrip() + f"\nSequence: {seq_expr}"
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
