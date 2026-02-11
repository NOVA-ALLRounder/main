from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from urllib.parse import urlparse
from typing import Optional


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build realtime LLM input from recent events")
    parser.add_argument("--config", default="", help="optional config path")
    parser.add_argument("--db", default="", help="override db path")
    parser.add_argument("--since-minutes", type=int, default=10)
    parser.add_argument("--output", default="logs/llm_input_realtime.json")
    parser.add_argument("--max-bytes", type=int, default=8000)
    parser.add_argument("--max-top-apps", type=int, default=5)
    parser.add_argument("--max-titles", type=int, default=5)
    parser.add_argument("--max-events", type=int, default=8)
    parser.add_argument("--max-samples", type=int, default=12)
    parser.add_argument("--max-sequence", type=int, default=12)
    parser.add_argument("--max-transitions", type=int, default=8)
    parser.add_argument("--min-duration-sec", type=int, default=5)
    parser.add_argument("--drop-idle", action="store_true")
    parser.add_argument("--summary-only", action="store_true")
    parser.add_argument(
        "--compression-level",
        default="balanced",
        choices=["strict", "balanced", "permissive"],
        help="compression level",
    )
    parser.add_argument(
        "--preserve-fields",
        default="",
        help="comma-separated fields to preserve during compression",
    )
    return parser.parse_args()


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _format_ts(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


def _load_config(args: argparse.Namespace):
    if not args.config:
        return None
    try:
        import sys

        project_root = Path(__file__).resolve().parents[1]
        sys.path.insert(0, str(project_root / "src"))
        from collector.config import load_config

        return load_config(Path(args.config))
    except Exception:
        return None


def _load_db_path(args: argparse.Namespace, config) -> Path:
    if args.db:
        return Path(args.db)
    if config is not None:
        try:
            return Path(config.db_path)
        except Exception:
            return Path("collector.db")
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


def _normalize_title(app: str, title: str) -> str:
    app_key = (app or "").lower()
    value = title.strip()
    suffix_map = {
        "chrome.exe": [" - Google Chrome", " - Chrome"],
        "msedge.exe": [" - Microsoft Edge", " - Edge"],
        "whale.exe": [" - Whale", " - Naver Whale"],
        "notion.exe": [" - Notion"],
        "code.exe": [
            " - Visual Studio Code",
            " - Visual Studio Code Insiders",
            " - Code",
        ],
    }
    for suffix in suffix_map.get(app_key, []):
        if value.endswith(suffix):
            value = value[: -len(suffix)].strip()
            break
    return value


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


def _infer_task_units(
    top_apps: list[dict],
    top_domains: list[dict],
    top_titles: list[dict],
    key_event_tokens: list[str],
) -> list[dict]:
    app_names = {_normalize_app_name(item.get("app")) for item in top_apps if item.get("app")}
    domain_names = {str(item.get("domain", "")).lower() for item in top_domains if item.get("domain")}
    title_blob = " ".join([str(item.get("title_hint", "")).lower() for item in top_titles if item.get("title_hint")])

    def add_unit(units: list[dict], unit_id: str, label: str, evidence: list[str], base: float = 0.6) -> None:
        confidence = min(0.95, base + 0.1 * len(evidence))
        units.append(
            {
                "unit_id": unit_id,
                "label": label,
                "confidence": round(confidence, 2),
                "evidence": evidence,
            }
        )

    units: list[dict] = []
    tokens = [t.lower() for t in key_event_tokens]

    doc_domains = {"notion.so", "docs.google.com", "confluence", "office.com", "onedrive.live.com"}
    comm_domains = {"slack.com", "teams.microsoft.com", "discord.com"}
    task_domains = {"jira", "linear.app", "trello.com", "asana.com", "clickup.com", "monday.com"}
    meet_domains = {"meet.google.com", "zoom.us", "webex.com"}
    mail_domains = {"mail.google.com", "outlook.live.com"}

    evidence = []
    if any(app in app_names for app in {"notion.exe"}):
        evidence.append("app:notion")
    if any(d in domain_names for d in doc_domains):
        evidence.append("domain:docs")
    if "doc" in title_blob or "note" in title_blob:
        evidence.append("title:doc")
    if evidence:
        add_unit(units, "doc_write", "Writing/Documentation", evidence)

    evidence = []
    if any(app in app_names for app in {"code.exe", "pycharm.exe", "idea.exe"}):
        evidence.append("app:code")
    if any(d in domain_names for d in {"github.com", "gitlab.com"}):
        evidence.append("domain:code_repo")
    if evidence:
        add_unit(units, "coding", "Coding/Development", evidence)

    evidence = []
    if any(app in app_names for app in {"slack.exe", "kakaotalk.exe"}):
        evidence.append("app:chat")
    if any(d in domain_names for d in comm_domains):
        evidence.append("domain:chat")
    if "message" in tokens or "chat" in tokens:
        evidence.append("event:message")
    if evidence:
        add_unit(units, "communication", "Communication", evidence)

    evidence = []
    if any(d in domain_names for d in task_domains):
        evidence.append("domain:task")
    if "task" in tokens or "jira" in tokens:
        evidence.append("event:task")
    if evidence:
        add_unit(units, "task_management", "Task Management", evidence)

    evidence = []
    if any(d in domain_names for d in meet_domains):
        evidence.append("domain:meeting")
    if "meet" in tokens or "call" in tokens:
        evidence.append("event:meeting")
    if evidence:
        add_unit(units, "meeting", "Meeting", evidence)

    evidence = []
    if any(d in domain_names for d in mail_domains):
        evidence.append("domain:mail")
    if "email" in tokens or "mail" in tokens:
        evidence.append("event:mail")
    if evidence:
        add_unit(units, "email", "Email/Follow-up", evidence)

    evidence = []
    if "search" in title_blob or "google.com" in domain_names or "wikipedia.org" in domain_names:
        evidence.append("domain:search")
    if evidence:
        add_unit(units, "research", "Research/Browsing", evidence, base=0.5)

    units.sort(key=lambda item: item.get("confidence", 0), reverse=True)
    return units[:4]


def _time_context(end_ts: datetime, tz_name: str, hour_counts: Counter[int]) -> dict:
    local_dt = end_ts.astimezone() if tz_name in {"", "local", "system", "default"} else None
    if tz_name and tz_name not in {"local", "system", "default"}:
        try:
            from zoneinfo import ZoneInfo

            local_dt = end_ts.astimezone(ZoneInfo(tz_name))
        except Exception:
            local_dt = end_ts.astimezone()
    if local_dt is None:
        local_dt = end_ts.astimezone()
    top_hours = [hour for hour, _ in hour_counts.most_common(3)]
    return {
        "hour_local": local_dt.hour,
        "weekday_local": local_dt.strftime("%a"),
        "active_hours": top_hours,
    }


def _pattern_repetition(transitions: Counter[tuple[str, str]], recent_sequence: list[str]) -> dict:
    max_transition = 0
    if transitions:
        max_transition = max(transitions.values())
    unique_apps = len({app for app in recent_sequence if app})
    repeat_ratio = 0.0
    if recent_sequence:
        repeat_ratio = 1.0 - (unique_apps / max(1, len(recent_sequence)))
    return {
        "transition_repeat_max": int(max_transition),
        "sequence_length": len(recent_sequence),
        "unique_apps": unique_apps,
        "repeat_ratio": round(repeat_ratio, 2),
    }


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
    return _compress_payload_with_level(payload, max_bytes, "balanced", set())


def _parse_preserve_fields(raw: str) -> set[str]:
    if not raw:
        return set()
    return {item.strip().lower() for item in raw.split(",") if item.strip()}


def _default_preserve_fields(level: str) -> set[str]:
    level = (level or "").lower().strip()
    if level == "strict":
        return {"top_domains", "recent_sequence", "app_transitions"}
    if level == "permissive":
        return {
            "top_titles",
            "recent_events",
            "top_domains",
            "top_pages",
            "focus_blocks",
            "recent_sequence",
            "app_transitions",
            "key_event_tokens",
        }
    return {"top_domains", "top_pages", "recent_sequence", "app_transitions", "key_event_tokens"}


def _prune_list(value: list, limit: int) -> list:
    if limit <= 0:
        return []
    return value[:limit]


def _compress_payload_with_level(payload: dict, max_bytes: int, level: str, preserve: set[str]) -> dict:
    if max_bytes <= 0:
        return payload

    def _size(val: dict) -> int:
        return len(json.dumps(val, ensure_ascii=False).encode("utf-8"))

    preserve_browser = "browser_context" in preserve
    preserve_domains = preserve_browser or "top_domains" in preserve
    preserve_pages = preserve_browser or "top_pages" in preserve
    preserve_transitions = "app_transitions" in preserve or "transitions" in preserve
    preserve_sequence = "recent_sequence" in preserve or "sequence" in preserve
    preserve_tokens = "key_event_tokens" in preserve or "tokens" in preserve
    preserve_focus = "focus_blocks" in preserve or "focus" in preserve

    compact = dict(payload)

    if level == "strict":
        if "top_titles" not in preserve:
            compact["top_titles"] = []
        if "recent_events" not in preserve:
            compact["recent_events"] = []
        browser_context = compact.get("browser_context") or {}
        if isinstance(browser_context, dict):
            browser_context = dict(browser_context)
            if not preserve_pages:
                browser_context["top_pages"] = []
            if not preserve_domains:
                browser_context["top_domains"] = _prune_list(browser_context.get("top_domains") or [], 3)
            compact["browser_context"] = browser_context
        if not preserve_transitions:
            compact["app_transitions"] = _prune_list(compact.get("app_transitions") or [], 4)
        if not preserve_sequence:
            compact["recent_sequence"] = _prune_list(compact.get("recent_sequence") or [], 6)
        if not preserve_tokens:
            compact["key_event_tokens"] = _prune_list(compact.get("key_event_tokens") or [], 8)
        if not preserve_focus:
            compact["focus_blocks"] = _prune_list(compact.get("focus_blocks") or [], 3)
    elif level == "balanced":
        if "recent_events" not in preserve:
            compact["recent_events"] = _prune_list(compact.get("recent_events") or [], 5)
        if "top_titles" not in preserve:
            compact["top_titles"] = _prune_list(compact.get("top_titles") or [], 5)
        browser_context = compact.get("browser_context") or {}
        if isinstance(browser_context, dict):
            browser_context = dict(browser_context)
            if not preserve_pages:
                browser_context["top_pages"] = _prune_list(browser_context.get("top_pages") or [], 3)
            if not preserve_domains:
                browser_context["top_domains"] = _prune_list(browser_context.get("top_domains") or [], 5)
            compact["browser_context"] = browser_context
    elif level == "permissive":
        # no pre-pruning, rely on size-based trimming below
        pass

    if _size(compact) <= max_bytes:
        return compact

    if "recent_events" not in preserve:
        compact["recent_events"] = _prune_list(compact.get("recent_events") or [], 5)
    browser_context = compact.get("browser_context") or {}
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_pages:
            browser_context["top_pages"] = _prune_list(browser_context.get("top_pages") or [], 3)
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    if not preserve_sequence:
        compact["recent_sequence"] = _prune_list(compact.get("recent_sequence") or [], 6)
    if not preserve_transitions:
        compact["app_transitions"] = _prune_list(compact.get("app_transitions") or [], 4)
    if not preserve_tokens:
        compact["key_event_tokens"] = _prune_list(compact.get("key_event_tokens") or [], 8)
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_pages:
            browser_context["top_pages"] = []
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    if "top_titles" not in preserve:
        compact["top_titles"] = []
    if isinstance(browser_context, dict):
        browser_context = dict(browser_context)
        if not preserve_domains:
            browser_context["top_domains"] = []
        compact["browser_context"] = browser_context
    if _size(compact) <= max_bytes:
        return compact

    compact["top_apps"] = _prune_list(compact.get("top_apps") or [], 3)
    compact["key_events"] = dict(list((compact.get("key_events") or {}).items())[:5])
    if not preserve_focus:
        compact["focus_blocks"] = _prune_list(compact.get("focus_blocks") or [], 3)
    if _size(compact) <= max_bytes:
        return compact

    compact["key_events"] = {}
    if "recent_events" not in preserve:
        compact["recent_events"] = []
    if not preserve_focus:
        compact["focus_blocks"] = []
    if not preserve_sequence:
        compact["recent_sequence"] = []
    if not preserve_transitions:
        compact["app_transitions"] = []
    if not preserve_tokens:
        compact["key_event_tokens"] = []
    compact["notes"] = ["compressed: reduced lists for size limit"]
    return compact


def main() -> None:
    args = parse_args()
    config = _load_config(args)
    db_path = _load_db_path(args, config)
    since_minutes = max(1, int(args.since_minutes))

    end_ts = _now_utc()
    start_ts = end_ts - timedelta(minutes=since_minutes)
    tz_name = "local"
    if config is not None:
        try:
            tz_name = str(config.logging.timezone or "local")
        except Exception:
            tz_name = "local"

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
    domain_counts: Counter[str] = Counter()
    page_counts: Counter[tuple[str, str]] = Counter()
    hour_counts: Counter[int] = Counter()
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
        ts_dt = None
        try:
            ts_dt = datetime.fromisoformat(str(ts).replace("Z", "+00:00"))
        except Exception:
            ts_dt = None
        if ts_dt is not None:
            local_dt = ts_dt.astimezone() if tz_name in {"", "local", "system", "default"} else None
            if local_dt is None and tz_name:
                try:
                    from zoneinfo import ZoneInfo

                    local_dt = ts_dt.astimezone(ZoneInfo(tz_name))
                except Exception:
                    local_dt = ts_dt.astimezone()
            if local_dt is None:
                local_dt = ts_dt.astimezone()
            hour_counts[local_dt.hour] += 1
        duration = payload.get("duration_sec")
        if isinstance(duration, (int, float)):
            if float(duration) < float(args.min_duration_sec):
                continue
            app_durations[app] += float(duration)
        title = str(payload.get("window_title") or "").strip()
        if title and not _contains_sensitive(title):
            title = _normalize_title(app, title)
            title = _trim_title(title, 60)
            title_counts[(app, title)] += 1
            if event_type_str == "os.app_focus_block":
                focus_blocks[(app, title)] += 1
        domain = payload.get("domain") or _safe_domain(payload.get("url") or payload.get("page_url") or "")
        if domain:
            domain_counts[str(domain).lower()] += 1
            if title and not _contains_sensitive(title):
                page_counts[(str(domain).lower(), title)] += 1

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
        url_domain = _safe_domain(payload.get("url") or payload.get("page_url") or payload.get("domain") or "")
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

    top_domains = []
    for domain, cnt in domain_counts.most_common(args.max_titles):
        top_domains.append({"domain": domain, "events": int(cnt)})

    top_pages = []
    for (domain, title), cnt in page_counts.most_common(args.max_titles):
        top_pages.append({"domain": domain, "title_hint": title, "events": int(cnt)})

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

    preserve_fields = _parse_preserve_fields(args.preserve_fields)
    preserve_fields |= _default_preserve_fields(args.compression_level)
    payload = {
        "generated_at": _format_ts(end_ts),
        "window": {"start_utc": _format_ts(start_ts), "end_utc": _format_ts(end_ts)},
        "time_context": _time_context(end_ts, tz_name, hour_counts),
        "quality": {
            "total_events": len(rows),
            "unique_apps": len(app_counts),
            "active_minutes": round(sum(app_durations.values()) / 60.0, 2),
            "window_minutes": since_minutes,
        },
        "compression": {
            "level": args.compression_level,
            "max_bytes": args.max_bytes,
            "preserve_fields": sorted(preserve_fields),
        },
        "top_apps": top_apps,
        "top_titles": top_titles,
        "browser_context": {
            "top_domains": top_domains,
            "top_pages": top_pages,
        },
        "focus_blocks": focus_block_list,
        "key_events": key_events,
        "key_event_tokens": [token for token, _ in key_event_tokens.most_common(12)],
        "app_transitions": transition_list,
        "recent_sequence": recent_sequence,
        "sequence_signature": _sequence_signature(recent_sequence),
        "recent_events": recent_events,
        "task_units": [],
        "task_summary": {},
        "pattern_repetition": {},
        "auto_approve": False,
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
    task_units = _infer_task_units(
        top_apps,
        top_domains,
        top_titles,
        [token for token, _ in key_event_tokens.most_common(12)],
    )
    payload["task_units"] = task_units
    if task_units:
        payload["task_summary"] = {
            "primary_task": task_units[0]["unit_id"],
            "label": task_units[0]["label"],
            "confidence": task_units[0]["confidence"],
        }

    repetition = _pattern_repetition(transitions, recent_sequence)
    payload["pattern_repetition"] = repetition
    auto_approve = bool(repetition.get("transition_repeat_max", 0) >= 2 and len(recent_sequence) >= 3)
    payload["auto_approve"] = auto_approve

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
    if auto_approve:
        hints = payload.get("workflow_hints") or {}
        hints["auto_approve"] = True
        payload["workflow_hints"] = hints

    if args.summary_only:
        payload["recent_event_stats"] = {
            "apps": [{"app": app, "events": int(cnt)} for app, cnt in app_counts.most_common(5)],
            "event_types": [
                {"event_type": evt, "events": int(cnt)}
                for evt, cnt in event_counts.most_common(5)
            ],
        }
        payload["recent_events"] = []

    payload = _compress_payload_with_level(payload, args.max_bytes, args.compression_level, preserve_fields)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"realtime_llm_input_saved={out_path}")


if __name__ == "__main__":
    main()
