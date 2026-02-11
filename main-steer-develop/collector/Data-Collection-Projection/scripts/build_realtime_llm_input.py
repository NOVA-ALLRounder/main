from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from urllib.parse import urlparse


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build realtime LLM input from recent events")
    parser.add_argument("--config", default="", help="optional config path")
    parser.add_argument("--db", default="", help="override db path")
    parser.add_argument("--since-minutes", type=int, default=10)
    parser.add_argument("--output", default="logs/run4/llm_input_realtime.json")
    parser.add_argument("--max-bytes", type=int, default=8000)
    parser.add_argument("--max-top-apps", type=int, default=5)
    parser.add_argument("--max-titles", type=int, default=5)
    parser.add_argument("--max-events", type=int, default=8)
    parser.add_argument("--max-samples", type=int, default=12)
    parser.add_argument("--max-sequence", type=int, default=12)
    parser.add_argument("--max-transitions", type=int, default=8)
    parser.add_argument("--min-duration-sec", type=int, default=5)
    parser.add_argument("--drop-idle", action="store_true")
    return parser.parse_args()


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _format_ts(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


def _load_db_path(args: argparse.Namespace) -> Path:
    if args.db:
        return Path(args.db)
    if args.config:
        try:
            import sys

            project_root = Path(__file__).resolve().parents[1]
            sys.path.insert(0, str(project_root / "src"))
            from collector.config import load_config

            config = load_config(Path(args.config))
            return Path(config.db_path)
        except Exception:
            pass
    return Path("collector.db")


def _safe_json(raw: str) -> dict:
    if not raw:
        return {}
    try:
        return json.loads(raw)
    except Exception:
        return {}


def _contains_sensitive(value: str) -> bool:
    import re

    if re.search(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}", value):
        return True
    if re.search(r"[A-Za-z]:\\\\|/Users/|/home/|/Volumes/", value):
        return True
    if re.search(r"https?://", value):
        return True
    if re.search(r"\\b\\d{12,}\\b", value):
        return True
    return False


def _safe_domain(url_value: str) -> str:
    if not url_value:
        return ""
    raw = str(url_value).strip()
    if not raw:
        return ""
    try:
        if "://" not in raw:
            raw = f"http://{raw}"
        parsed = urlparse(raw)
        domain = parsed.netloc or ""
        return domain.lower()
    except Exception:
        return ""


def _trim_title(value: str, max_len: int) -> str:
    if max_len <= 0:
        return value
    if len(value) <= max_len:
        return value
    return value[: max_len - 1] + "?"


def _tokenize_event(name: str) -> list[str]:
    import re

    tokens = re.split(r"[^A-Za-z0-9]+", name or "")
    stop = {"os", "app", "focus", "block", "idle", "event", "window"}
    out = []
    for token in tokens:
        if not token:
            continue
        t = token.lower()
        if t in stop or len(t) <= 2:
            continue
        out.append(t)
    return out


def _normalize_app_name(name: str) -> str:
    return str(name or "").strip().lower()


def _infer_intents(
    top_apps: list[dict],
    key_event_tokens: list[str],
    transitions: list[dict],
    recent_sequence: list[str],
) -> list[dict]:
    app_names = {_normalize_app_name(item.get("app")) for item in top_apps if item.get("app")}
    seq_apps = {_normalize_app_name(a) for a in recent_sequence}
    app_names |= seq_apps

    def _hit(token_list: list[str], *needles: str) -> bool:
        for needle in needles:
            if needle in token_list:
                return True
        return False

    intents: list[dict] = []

    def add_intent(intent_id: str, label: str, evidence: list[str], base: float = 0.6) -> None:
        confidence = min(0.95, base + 0.1 * len(evidence))
        intents.append(
            {
                "intent_id": intent_id,
                "label": label,
                "confidence": round(confidence, 2),
                "evidence": evidence,
            }
        )

    tokens = [t.lower() for t in key_event_tokens]
    evidence: list[str] = []
    if _hit(tokens, "summary", "report", "daily"):
        evidence.append("key_event_tokens")
    if "notion" in " ".join(app_names):
        evidence.append("app:notion")
    if evidence:
        add_intent("daily_summary", "Daily summary/report", evidence)

    evidence = []
    if _hit(tokens, "slack", "notify", "message"):
        evidence.append("key_event_tokens")
    if any("slack" in app for app in app_names):
        evidence.append("app:slack")
    if evidence:
        add_intent("team_update", "Team update/notification", evidence)

    evidence = []
    if _hit(tokens, "email", "gmail", "outlook", "followup"):
        evidence.append("key_event_tokens")
    if any("outlook" in app or "gmail" in app for app in app_names):
        evidence.append("app:mail")
    if evidence:
        add_intent("followup_email", "Draft follow-up email", evidence)

    evidence = []
    if _hit(tokens, "file", "save", "export", "upload"):
        evidence.append("key_event_tokens")
    if evidence:
        add_intent("file_backup", "File save/export follow-up", evidence)

    # Heuristic: frequent app switching -> focus recap
    if transitions:
        add_intent(
            "focus_recap",
            "Focus recap from app switching",
            ["app_transitions"],
            base=0.5,
        )

    intents.sort(key=lambda item: item.get("confidence", 0), reverse=True)
    return intents[:3]


def _workflow_hints(intents: list[dict]) -> dict:
    if not intents:
        return {}
    primary = intents[0]["intent_id"]
    template_map = {
        "daily_summary": "daily_summary_to_notion",
        "team_update": "focus_report_to_slack",
        "followup_email": "followup_email_draft",
        "file_backup": "file_save_followup",
        "focus_recap": "focus_report_to_slack",
    }
    required_tools_map = {
        "daily_summary": ["notion"],
        "team_update": ["slack"],
        "followup_email": ["gmail"],
        "file_backup": ["notion"],
        "focus_recap": ["slack"],
    }
    return {
        "recommended_template": template_map.get(primary, ""),
        "required_tools": required_tools_map.get(primary, []),
    }


def _sequence_signature(recent_sequence: list[str]) -> str:
    if not recent_sequence:
        return ""
    trimmed = [app for app in recent_sequence if app]
    if len(trimmed) <= 1:
        return trimmed[0] if trimmed else ""
    return " -> ".join(trimmed[:6])


def _compress_payload(payload: dict, max_bytes: int) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(val: dict) -> int:
        return len(json.dumps(val, ensure_ascii=False).encode("utf-8"))

    if _size(payload) <= max_bytes:
        return payload

    compact = dict(payload)
    compact["recent_events"] = (compact.get("recent_events") or [])[:5]
    if _size(compact) <= max_bytes:
        return compact

    compact["recent_sequence"] = (compact.get("recent_sequence") or [])[:6]
    compact["app_transitions"] = (compact.get("app_transitions") or [])[:4]
    compact["key_event_tokens"] = (compact.get("key_event_tokens") or [])[:8]
    if _size(compact) <= max_bytes:
        return compact

    compact["top_titles"] = []
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = (compact.get("top_apps") or [])[:3]
    compact["key_events"] = dict(list((compact.get("key_events") or {}).items())[:5])
    compact["focus_blocks"] = (compact.get("focus_blocks") or [])[:3]
    if _size(compact) <= max_bytes:
        return compact

    compact["key_events"] = {}
    compact["recent_events"] = []
    compact["focus_blocks"] = []
    compact["recent_sequence"] = []
    compact["app_transitions"] = []
    compact["key_event_tokens"] = []
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def main() -> None:
    args = parse_args()
    db_path = _load_db_path(args)
    since_minutes = max(1, int(args.since_minutes))

    end_ts = _now_utc()
    start_ts = end_ts - timedelta(minutes=since_minutes)

    conn = sqlite3.connect(str(db_path))
    cur = conn.cursor()
    rows = cur.execute(
        """
        SELECT ts, app, event_type, priority, source, resource_type, resource_id, payload_json
        FROM events WHERE ts >= ? ORDER BY ts ASC
        """,
        (_format_ts(start_ts),),
    ).fetchall()
    conn.close()

    app_durations: dict[str, float] = defaultdict(float)
    app_counts: Counter[str] = Counter()
    title_counts: Counter[tuple[str, str]] = Counter()
    event_counts: Counter[str] = Counter()
    focus_blocks: Counter[tuple[str, str]] = Counter()
    recent_events: list[dict] = []
    transitions: Counter[tuple[str, str]] = Counter()
    recent_sequence: list[str] = []
    last_focus_app: str | None = None

    for ts, app, event_type, priority, source, resource_type, resource_id, payload_json in rows:
        app = str(app or "unknown")
        app_counts[app] += 1
        event_type_str = str(event_type or "unknown")
        if args.drop_idle and event_type_str.startswith("os.idle_"):
            continue
        event_counts[event_type_str] += 1
        payload = _safe_json(payload_json)
        duration = payload.get("duration_sec")
        if isinstance(duration, (int, float)):
            if float(duration) < float(args.min_duration_sec):
                continue
            app_durations[app] += float(duration)
        title = str(payload.get("window_title") or "").strip()
        if title and not _contains_sensitive(title):
            title = _trim_title(title, 60)
            title_counts[(app, title)] += 1
            if event_type_str == "os.app_focus_block":
                focus_blocks[(app, title)] += 1

        if "focus" in event_type_str or "foreground" in event_type_str:
            if last_focus_app and last_focus_app != app:
                transitions[(last_focus_app, app)] += 1
            if not recent_sequence or recent_sequence[-1] != app:
                recent_sequence.append(app)
            last_focus_app = app

        sample = {
            "ts": ts,
            "app": app,
            "event_type": event_type_str,
        }
        if priority:
            sample["priority"] = priority
        if source:
            sample["source"] = source
        if resource_type:
            sample["resource_type"] = resource_type
        if resource_id and not _contains_sensitive(str(resource_id)):
            sample["resource_id"] = str(resource_id)[:64]
        if isinstance(duration, (int, float)):
            sample["duration_sec"] = round(float(duration), 2)
        if title and not _contains_sensitive(title):
            sample["title_hint"] = title
        url_domain = _safe_domain(payload.get("url") or payload.get("page_url") or "")
        if url_domain:
            sample["url_domain"] = url_domain
        if payload.get("action"):
            sample["action"] = str(payload.get("action"))[:64]
        recent_events.append(sample)

    top_apps = []
    if app_durations:
        for app, total_sec in sorted(app_durations.items(), key=lambda i: i[1], reverse=True)[: args.max_top_apps]:
            top_apps.append({"app": app, "minutes": round(total_sec / 60.0, 2), "events": app_counts[app]})
    else:
        for app, cnt in app_counts.most_common(args.max_top_apps):
            top_apps.append({"app": app, "events": cnt})

    top_titles = []
    for (app, title), cnt in title_counts.most_common(args.max_titles):
        top_titles.append({"app": app, "title_hint": title, "events": cnt})

    key_events = dict(event_counts.most_common(args.max_events))
    focus_block_list = []
    for (app, title), cnt in focus_blocks.most_common(args.max_titles):
        focus_block_list.append({"app": app, "title_hint": title, "blocks": cnt})

    transition_list = []
    for (app_from, app_to), cnt in transitions.most_common(args.max_transitions):
        transition_list.append({"from": app_from, "to": app_to, "count": int(cnt)})

    key_event_tokens = Counter()
    for event_name in key_events.keys():
        for token in _tokenize_event(event_name):
            key_event_tokens[token] += 1

    if args.max_samples > 0:
        recent_events = recent_events[-args.max_samples :]
    if args.max_sequence > 0:
        recent_sequence = recent_sequence[-args.max_sequence :]

    payload = {
        "generated_at": _format_ts(end_ts),
        "window": {"start_utc": _format_ts(start_ts), "end_utc": _format_ts(end_ts)},
        "quality": {
            "total_events": len(rows),
            "unique_apps": len(app_counts),
            "active_minutes": round(sum(app_durations.values()) / 60.0, 2),
            "window_minutes": since_minutes,
        },
        "top_apps": top_apps,
        "top_titles": top_titles,
        "focus_blocks": focus_block_list,
        "key_events": key_events,
        "key_event_tokens": [token for token, _ in key_event_tokens.most_common(12)],
        "app_transitions": transition_list,
        "recent_sequence": recent_sequence,
        "sequence_signature": _sequence_signature(recent_sequence),
        "recent_events": recent_events,
        "intent_candidates": [],
        "intent_summary": {},
        "workflow_hints": {},
        "source": "realtime",
        "notes": [
            "Recent events only; use with pattern summaries for long-term context.",
            "Sensitive titles are redacted.",
        ],
    }

    intents = _infer_intents(top_apps, payload["key_event_tokens"], transition_list, recent_sequence)
    payload["intent_candidates"] = intents
    if intents:
        payload["intent_summary"] = {
            "primary_intent": intents[0]["intent_id"],
            "confidence": intents[0]["confidence"],
            "label": intents[0]["label"],
        }
        payload["workflow_hints"] = _workflow_hints(intents)

    # If we have a meaningful sequence, add a default hint for sequence-based workflows.
    if len(recent_sequence) >= 3:
        hints = payload.get("workflow_hints") or {}
        # Prefer sequence-based template when we have a clear sequence.
        hints["recommended_template"] = "sequence_triggered_notification"
        required_tools = hints.get("required_tools") or []
        if not required_tools:
            required_tools = ["slack"]
        hints["required_tools"] = required_tools
        hints["sequence_based"] = True
        payload["workflow_hints"] = hints

    payload = _compress_payload(payload, args.max_bytes)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"realtime_llm_input_saved={out_path}")


if __name__ == "__main__":
    main()
