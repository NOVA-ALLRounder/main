from __future__ import annotations

import argparse
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Score n8n workflow quality")
    parser.add_argument("--file", required=True, help="workflow json path")
    parser.add_argument("--profile", default="", help="personalization profile json")
    parser.add_argument("--score-config", default="", help="score config json path")
    parser.add_argument("--llm-input", default="", help="llm_input json path")
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
        base = _load_json(path)
    else:
        default_path = Path(__file__).resolve().parents[1] / "configs" / "personalization_demo.json"
        base = _load_json(str(default_path))
    learned_path = Path(__file__).resolve().parents[1] / "configs" / "profile_learned.json"
    learned = _load_json(str(learned_path))
    if not isinstance(base, dict):
        base = {}
    if not isinstance(learned, dict) or not learned:
        return base
    merged = json.loads(json.dumps(base))
    if learned.get("tools"):
        merged["tools"] = learned.get("tools")
    if learned.get("preferences"):
        prefs = merged.get("preferences") or {}
        if not isinstance(prefs, dict):
            prefs = {}
        prefs.update(learned.get("preferences"))
        merged["preferences"] = prefs
    return merged


def _load_score_config(path: str) -> dict:
    if path:
        return _load_json(path)
    default_path = Path(__file__).resolve().parents[1] / "configs" / "score_config.json"
    return _load_json(str(default_path))


def _load_llm_input(path: str) -> dict:
    if not path:
        return {}
    return _load_json(path)


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


def _has_input_usage(nodes: list[dict]) -> bool:
    for node in nodes:
        if not isinstance(node, dict):
            continue
        params = node.get("parameters") or {}
        try:
            blob = json.dumps(params, ensure_ascii=False)
        except Exception:
            continue
        if "$json" in blob or "key_events" in blob or "recent_events" in blob:
            return True
    return False


def _expected_tools(llm_input: dict) -> list[str]:
    if not isinstance(llm_input, dict):
        return []
    hints = llm_input.get("workflow_hints") or {}
    if isinstance(hints, dict) and hints.get("required_tools"):
        return [str(t).lower() for t in hints.get("required_tools") if str(t).strip()]
    intent_summary = llm_input.get("intent_summary") or {}
    primary = intent_summary.get("primary_intent")
    mapping = {
        "daily_summary": ["notion"],
        "team_update": ["slack"],
        "followup_email": ["gmail"],
        "file_backup": ["notion"],
        "focus_recap": ["slack"],
    }
    return mapping.get(primary, [])


def _expects_sequence(llm_input: dict) -> bool:
    if not isinstance(llm_input, dict):
        return False
    recent_sequence = llm_input.get("recent_sequence") or []
    transitions = llm_input.get("app_transitions") or []
    return (isinstance(recent_sequence, list) and len(recent_sequence) >= 3) or bool(transitions)


def _has_sequence_usage(nodes: list[dict]) -> bool:
    for node in nodes:
        if not isinstance(node, dict):
            continue
        params = node.get("parameters") or {}
        try:
            blob = json.dumps(params, ensure_ascii=False).lower()
        except Exception:
            continue
        if "recent_sequence" in blob or "app_transitions" in blob or "sequence_signature" in blob:
            return True
    return False


def _count_conditions(node: dict) -> int:
    if not isinstance(node, dict):
        return 0
    params = node.get("parameters") or {}
    conditions = params.get("conditions") or {}
    if not isinstance(conditions, dict):
        return 0
    total = 0
    for value in conditions.values():
        if isinstance(value, list):
            total += len(value)
    return total


def _conditions_blob(if_nodes: list[dict]) -> str:
    try:
        return json.dumps([node.get("parameters", {}) for node in if_nodes], ensure_ascii=False).lower()
    except Exception:
        return ""


def _has_condition_tokens(if_nodes: list[dict], tokens: list[str]) -> bool:
    blob = _conditions_blob(if_nodes)
    return any(token in blob for token in tokens)


def _recommended_template(llm_input: dict) -> str:
    if not isinstance(llm_input, dict):
        return ""
    hints = llm_input.get("workflow_hints") or {}
    if isinstance(hints, dict):
        return str(hints.get("recommended_template") or "")
    return ""


def _meta_note(data: dict) -> str:
    if not isinstance(data, dict):
        return ""
    meta = data.get("meta") or {}
    if isinstance(meta, dict):
        return str(meta.get("note") or "")
    return ""


def _validate_connections(data: dict) -> tuple[bool, list[str]]:
    nodes = data.get("nodes") if isinstance(data, dict) else []
    if not isinstance(nodes, list):
        nodes = []
    node_names = {n.get("name") for n in nodes if isinstance(n, dict) and n.get("name")}
    node_ids = {n.get("id") for n in nodes if isinstance(n, dict) and n.get("id")}
    connections = data.get("connections") if isinstance(data, dict) else {}
    if not isinstance(connections, dict):
        return False, ["connections_not_dict"]

    issues = []
    for source, link in connections.items():
        if source not in node_names and source not in node_ids:
            issues.append(f"unknown_source:{source}")
        if not isinstance(link, dict):
            continue
        for branch in link.get("main") or []:
            if not isinstance(branch, list):
                continue
            for item in branch:
                if not isinstance(item, dict):
                    continue
                target = item.get("node")
                if target and target not in node_names and target not in node_ids:
                    issues.append(f"unknown_target:{target}")
    return len(issues) == 0, issues


def main() -> None:
    args = parse_args()
    data = _load_json(args.file)
    profile = _load_profile(args.profile)
    score_cfg = _load_score_config(args.score_config) or {}
    llm_input = _load_llm_input(args.llm_input)
    weights = score_cfg.get("weights") or {}

    score = 0
    notes = []

    nodes = data.get("nodes") if isinstance(data, dict) else []
    if not isinstance(nodes, list):
        nodes = []
    node_count = len(nodes)
    simple_mode = _is_simple_context(llm_input)

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

    if simple_mode and node_count and node_count <= int(weights.get("simple_flow_max_nodes", 3)) and has_trigger:
        score += int(weights.get("simple_flow_bonus", 5))
    over_complex_nodes = int(weights.get("over_complex_nodes", 9))
    over_complex_nodes2 = int(weights.get("over_complex_nodes2", 14))
    if node_count >= over_complex_nodes2:
        score += int(weights.get("over_complex_penalty2", -15))
    elif node_count >= over_complex_nodes:
        score += int(weights.get("over_complex_penalty", -10))

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
            penalty = int(weights.get("tool_missing_penalty", -20))
            if simple_mode:
                penalty = int(weights.get("tool_missing_penalty_simple", -5))
            score += penalty
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
    condition_count = sum(_count_conditions(n) for n in if_nodes)
    min_conditions = int(weights.get("min_conditions", 2))
    if not simple_mode:
        if not if_nodes:
            score += int(weights.get("missing_if_penalty", -6))
            notes.append("missing_if_node")
        elif condition_count < min_conditions:
            score += int(weights.get("under_condition_penalty", -8))
            notes.append("under_conditioned_if")
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

    # Condition coverage expectations
    expects_time = False
    prefs = profile.get("preferences") if isinstance(profile, dict) else {}
    if isinstance(prefs, dict) and prefs.get("working_hours"):
        expects_time = True
    time_context = llm_input.get("time_context") if isinstance(llm_input, dict) else {}
    if isinstance(time_context, dict) and time_context.get("active_hours"):
        expects_time = True
    expects_sequence = _expects_sequence(llm_input)
    expects_frequency = bool((llm_input.get("key_events") or {})) or bool(
        (llm_input.get("quality") or {}).get("active_minutes")
    )
    safety = profile.get("safety") if isinstance(profile, dict) else {}
    expects_approval = bool(safety.get("require_approval")) or bool(
        llm_input.get("auto_approve") if isinstance(llm_input, dict) else False
    )

    has_time_condition = _has_condition_tokens(if_nodes, ["$now.hour", "working_hours", "hour"])
    has_sequence_condition = _has_condition_tokens(
        if_nodes, ["sequence_signature", "recent_sequence", "app_transitions"]
    )
    has_frequency_condition = _has_condition_tokens(
        if_nodes, ["key_events", "active_minutes", "total_events"]
    )
    has_approval_condition = _has_condition_tokens(if_nodes, ["auto_approve", "approval"])

    if expects_time:
        if has_time_condition:
            score += int(weights.get("time_condition_bonus", 4))
        else:
            score += int(weights.get("time_condition_missing_penalty", -6))
            notes.append("missing_time_condition")

    if expects_sequence:
        if has_sequence_condition:
            score += int(weights.get("sequence_condition_bonus", 4))
        else:
            score += int(weights.get("sequence_condition_missing_penalty", -6))
            notes.append("missing_sequence_condition")

    if expects_frequency and not simple_mode:
        if has_frequency_condition:
            score += int(weights.get("frequency_condition_bonus", 4))
        else:
            score += int(weights.get("frequency_condition_missing_penalty", -6))
            notes.append("missing_frequency_condition")

    if expects_approval:
        if has_approval_condition:
            score += int(weights.get("approval_condition_bonus", 4))
        else:
            score += int(weights.get("approval_condition_missing_penalty", -8))
            notes.append("missing_approval_condition")

    # Action node presence (Notion/Slack/Gmail/HTTP)
    action_nodes = [t for t in node_types if any(x in t for x in ["notion", "slack", "gmail", "http"])]
    if action_nodes:
        score += int(weights.get("action_node", 10))
    else:
        notes.append("no_action_nodes")

    # Input usage (only if webhook trigger present)
    uses_webhook = any("webhook" in t for t in node_types)
    if uses_webhook and node_count > 1:
        if _has_input_usage(nodes):
            score += int(weights.get("input_usage_bonus", 5))
        else:
            penalty = int(weights.get("input_unused_penalty", -10))
            if simple_mode:
                penalty = int(weights.get("input_unused_penalty_simple", -3))
            score += penalty
            notes.append("no_input_usage")

    # Intent/tool alignment (avoid irrelevant action tools)
    expected_tools = _expected_tools(llm_input)
    if expected_tools:
        if any(tool in str(t).lower() for t in node_types for tool in expected_tools):
            score += int(weights.get("intent_tool_bonus", 5))
        else:
            penalty = int(weights.get("intent_tool_missing_penalty", -10))
            if simple_mode:
                penalty = int(weights.get("intent_tool_missing_penalty_simple", -3))
            score += penalty
            notes.append("intent_tool_mismatch")

    # Template hint alignment
    recommended = _recommended_template(llm_input)
    if recommended:
        note = _meta_note(data)
        source = (data.get("meta") or {}).get("source") if isinstance(data, dict) else ""
        if note and recommended in note:
            score += int(weights.get("template_match_bonus", 4))
        elif source == "template":
            score += int(weights.get("template_mismatch_penalty", -6))
            notes.append("template_hint_mismatch")

    # Sequence usage when sequence hints exist
    if _expects_sequence(llm_input):
        if _has_sequence_usage(nodes):
            score += int(weights.get("sequence_usage_bonus", 5))
        else:
            penalty = int(weights.get("sequence_unused_penalty", -10))
            if simple_mode:
                penalty = int(weights.get("sequence_unused_penalty_simple", -3))
            score += penalty
            notes.append("sequence_not_used")

    # Connection sanity (avoid disconnected multi-node workflows)
    if node_count > 1 and not connections:
        score += int(weights.get("empty_connections_penalty", -10))
        notes.append("missing_connections")

    # Import validation (node/connection integrity)
    valid_connections, conn_issues = _validate_connections(data)
    if node_count > 1 and not connections:
        valid_connections = False
        conn_issues.append("missing_connections")
    if valid_connections:
        score += int(weights.get("import_valid_bonus", 6))
    else:
        score += int(weights.get("import_invalid_penalty", -12))
        for issue in conn_issues:
            notes.append(f"invalid_connection:{issue}")

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
