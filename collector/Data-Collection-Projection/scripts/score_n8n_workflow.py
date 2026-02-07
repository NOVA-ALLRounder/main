from __future__ import annotations

import argparse
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Score n8n workflow quality")
    parser.add_argument("--file", required=True, help="workflow json path")
    parser.add_argument("--profile", default="", help="personalization profile json")
    parser.add_argument("--score-config", default="", help="score config json path")
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
    default_path = Path(__file__).resolve().parents[1] / "configs" / "personalization_demo.json"
    return _load_json(str(default_path))


def _load_score_config(path: str) -> dict:
    if path:
        return _load_json(path)
    default_path = Path(__file__).resolve().parents[1] / "configs" / "score_config.json"
    return _load_json(str(default_path))


def main() -> None:
    args = parse_args()
    data = _load_json(args.file)
    profile = _load_profile(args.profile)
    score_cfg = _load_score_config(args.score_config) or {}
    weights = score_cfg.get("weights") or {}

    score = 0
    notes = []

    nodes = data.get("nodes") if isinstance(data, dict) else []
    if not isinstance(nodes, list):
        nodes = []

    # Base structure (minimal)
    if data.get("name"):
        score += int(weights.get("name", 10))
    if nodes:
        score += int(weights.get("nodes", 10))

    node_types = [node.get("type", "") for node in nodes if isinstance(node, dict)]
    has_trigger = any("webhook" in t or "cron" in t for t in node_types)
    if has_trigger:
        score += int(weights.get("trigger", 15))
    else:
        notes.append("missing_trigger")

    # Placeholder detection
    placeholder = False
    for node in nodes:
        if not isinstance(node, dict):
            continue
        params = node.get("parameters") or {}
        url = params.get("url") if isinstance(params, dict) else None
        if isinstance(url, str) and "example.com" in url:
            placeholder = True
        if isinstance(params, dict):
            for key in ("databaseId", "channel", "toEmail"):
                val = params.get(key)
                if isinstance(val, str) and "your_" in val:
                    placeholder = True
    if placeholder:
        score += int(weights.get("placeholder_penalty", -40))
        notes.append("placeholder_url_detected")

    # Profile tool coverage (partial credit)
    tools = [t.lower() for t in (profile.get("tools") or [])]
    if tools:
        matched_tools = set()
        for t in node_types:
            t_low = str(t).lower()
            for tool in tools:
                if tool in t_low:
                    matched_tools.add(tool)
        if matched_tools:
            per = int(weights.get("tool_match_per", 10))
            max_w = int(weights.get("tool_match_max", 30))
            score += min(max_w, per * len(matched_tools))
        else:
            score += int(weights.get("tool_missing_penalty", -20))
            notes.append("no_profile_tool_nodes")

    # Profile ids usage (partial credit)
    ids = profile.get("ids") or {}
    if ids:
        uses_ids = set()
        key_map = {
            "notion_database_id": "databaseId",
            "slack_channel": "channel",
            "gmail_to": "toEmail",
        }
        for node in nodes:
            if not isinstance(node, dict):
                continue
            params = node.get("parameters") or {}
            if not isinstance(params, dict):
                continue
            for key, expected in ids.items():
                param_key = key_map.get(key, key)
                if isinstance(expected, str) and expected and params.get(param_key) == expected:
                    uses_ids.add(key)
        if uses_ids:
            per = int(weights.get("id_match_per", 5))
            max_w = int(weights.get("id_match_max", 15))
            score += min(max_w, per * len(uses_ids))
        else:
            notes.append("no_profile_ids_usage")

    # Key events usage (partial credit)
    key_event_hits = 0
    for node in nodes:
        if not isinstance(node, dict):
            continue
        blob = json.dumps(node.get("parameters", {}), ensure_ascii=False)
        if "key_events" in blob:
            key_event_hits += 1
    if key_event_hits > 0:
        base = int(weights.get("key_events_base", 10))
        per = int(weights.get("key_events_per", 5))
        max_w = int(weights.get("key_events_max", 20))
        score += min(max_w, base + (key_event_hits - 1) * per)
    else:
        notes.append("no_key_events_usage")

    # Time-window usage
    uses_time_window = any(
        any(
            token in json.dumps(node.get("parameters", {}), ensure_ascii=False).lower()
            for token in ["working_hours", "time", "hour", "$now"]
        )
        for node in nodes
        if isinstance(node, dict)
    )
    if uses_time_window:
        score += int(weights.get("time_window", 15))
    else:
        notes.append("no_time_window_usage")

    # IF node branching
    if_nodes = [n for n in nodes if isinstance(n, dict) and "if" in str(n.get("type", "")).lower()]
    has_branching = False
    connections = data.get("connections") if isinstance(data, dict) else {}
    if isinstance(connections, dict) and if_nodes:
        for n in if_nodes:
            name = n.get("name")
            node_id = n.get("id")
            key = None
            if name and name in connections:
                key = name
            elif node_id and node_id in connections:
                key = node_id
            if not key:
                continue
            branches = connections.get(key) or {}
            if isinstance(branches, dict):
                blob = json.dumps(branches, ensure_ascii=False).lower()
                if "true" in blob and "false" in blob:
                    has_branching = True
                    break
    # Branching is optional for simple flows, no penalty if absent
    if has_branching:
        score += int(weights.get("if_branching", 5))

    # Action node presence (Notion/Slack/Gmail/HTTP)
    action_nodes = [t for t in node_types if any(x in t for x in ["notion", "slack", "gmail", "http"])]
    if action_nodes:
        score += int(weights.get("action_node", 10))
    else:
        notes.append("no_action_nodes")

    # Safety: allow_actions enforcement (no penalty if not provided)
    safety = profile.get("safety") if isinstance(profile, dict) else {}
    allow_actions = [a.lower() for a in (safety.get("allow_actions") or [])]
    if allow_actions:
        disallowed = []
        for t in action_nodes:
            t_low = str(t).lower()
            if not any(a in t_low for a in allow_actions):
                disallowed.append(t)
        if disallowed:
            score += int(weights.get("allow_actions_penalty", -15))
            notes.append("disallowed_action_nodes")

    # Safety: require approval via IF gate
    if safety.get("require_approval"):
        if not if_nodes:
            score += int(weights.get("approval_missing_penalty", -10))
            notes.append("missing_approval_gate")

    score = max(0, min(100, score))
    print(f"score={score}")
    if notes:
        print("notes:")
        for note in notes:
            print(f"- {note}")


if __name__ == "__main__":
    main()
