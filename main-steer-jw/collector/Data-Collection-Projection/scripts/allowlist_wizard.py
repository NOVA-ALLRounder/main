from __future__ import annotations

import argparse
import csv
import io
import json
import sqlite3
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import timedelta
from pathlib import Path
from typing import Any, Dict, Iterable

import yaml

# Ensure local src is importable when running as a script.
PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config
from collector.utils.time import utc_now


@dataclass
class AppStats:
    app: str
    seconds: float = 0.0
    blocks: int = 0
    last_seen_ts: str = ""
    sources: set[str] = field(default_factory=set)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build allowlist candidates from installed/running/observed apps"
    )
    parser.add_argument(
        "--config", default="configs/config.yaml", help="path to config file"
    )
    parser.add_argument(
        "--days", type=float, default=3.0, help="look back N days for usage"
    )
    parser.add_argument(
        "--min-minutes",
        type=float,
        default=10.0,
        help="min focus minutes to recommend",
    )
    parser.add_argument(
        "--min-blocks", type=int, default=3, help="min focus blocks to recommend"
    )
    parser.add_argument(
        "--output",
        default="configs/allowlist_selection.yaml",
        help="path to write selection template",
    )
    parser.add_argument(
        "--apply-recommended",
        action="store_true",
        help="apply recommended apps to allowlist immediately",
    )
    parser.add_argument(
        "--apply-selection",
        default="",
        help="path to selection YAML to apply (allow/deny lists)",
    )
    parser.add_argument(
        "--include-installed",
        action="store_true",
        help="include installed app candidates",
    )
    parser.add_argument(
        "--include-running",
        action="store_true",
        help="include running process candidates",
    )
    parser.add_argument(
        "--include-observed",
        action="store_true",
        help="include observed apps from focus blocks",
    )
    parser.add_argument(
        "--top-n", type=int, default=200, help="limit candidates"
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config = load_config(args.config)
    privacy_rules_path = Path(config.privacy_rules_path)

    allowlist, denylist = _load_privacy_lists(privacy_rules_path)

    include_observed = bool(args.include_observed) or (
        not args.include_installed and not args.include_running
    )
    candidates: dict[str, AppStats] = {}

    start_ts = None
    if include_observed and args.days:
        start_ts = (utc_now() - timedelta(days=args.days)).isoformat().replace(
            "+00:00", "Z"
        )
        _merge_stats(
            candidates,
            _collect_focus_stats(config.db_path, start_ts),
            source="observed",
        )

    if args.include_running:
        _merge_names(candidates, _collect_running_exes(), source="running")

    if args.include_installed:
        _merge_names(candidates, _collect_installed_exes(), source="installed")

    items = list(candidates.values())
    items = _sort_candidates(items)
    if args.top_n and args.top_n > 0:
        items = items[: args.top_n]

    recommended = _build_recommendations(
        items, args.min_minutes, args.min_blocks, allowlist, denylist
    )

    output_path = Path(args.output)
    report = _build_report(
        items,
        recommended,
        allowlist,
        denylist,
        start_ts=start_ts,
        min_minutes=args.min_minutes,
        min_blocks=args.min_blocks,
    )
    output_path.write_text(
        yaml.safe_dump(report, sort_keys=False, allow_unicode=False)
    )
    print(f"selection_template={output_path}")
    print(f"candidates={len(items)} recommended={len(recommended)}")

    if args.apply_recommended:
        added = _apply_allowlist_update(privacy_rules_path, allowlist, denylist, recommended)
        if added:
            print(f"allowlist_applied={len(added)} backup={privacy_rules_path}.bak")
        else:
            print("allowlist_apply_skipped=none")

    if args.apply_selection:
        selection_path = Path(args.apply_selection)
        if not selection_path.exists():
            raise FileNotFoundError(f"selection file not found: {selection_path}")
        selection_raw = yaml.safe_load(selection_path.read_text()) or {}
        selected_allow = _lower_list(selection_raw.get("selection", {}).get("allow"))
        selected_deny = _lower_list(selection_raw.get("selection", {}).get("deny"))
        added_allow, added_deny = _apply_selection(
            privacy_rules_path, allowlist, denylist, selected_allow, selected_deny
        )
        print(
            f"selection_applied allow_added={len(added_allow)} deny_added={len(added_deny)} backup={privacy_rules_path}.bak"
        )


def _merge_stats(
    base: dict[str, AppStats], stats: dict[str, AppStats], source: str
) -> None:
    for app_key, entry in stats.items():
        existing = base.get(app_key)
        if existing is None:
            entry.sources.add(source)
            base[app_key] = entry
            continue
        existing.seconds += entry.seconds
        existing.blocks += entry.blocks
        if entry.last_seen_ts and entry.last_seen_ts > existing.last_seen_ts:
            existing.last_seen_ts = entry.last_seen_ts
        existing.sources.add(source)


def _merge_names(base: dict[str, AppStats], names: Iterable[str], source: str) -> None:
    for name in names:
        app_key = _normalize_app(name)
        if not app_key:
            continue
        entry = base.get(app_key)
        if entry is None:
            entry = AppStats(app=app_key)
            base[app_key] = entry
        entry.sources.add(source)


def _collect_focus_stats(db_path: Path, start_ts: str | None) -> dict[str, AppStats]:
    if not Path(db_path).exists():
        raise FileNotFoundError(f"db not found: {db_path}")

    query = "SELECT ts, app, payload_json FROM events WHERE event_type = ?"
    params: list[Any] = ["os.app_focus_block"]
    if start_ts:
        query += " AND ts >= ?"
        params.append(start_ts)
    query += " ORDER BY ts ASC"

    stats: dict[str, AppStats] = {}
    conn = sqlite3.connect(str(db_path))
    try:
        for ts, app, payload_json in conn.execute(query, params):
            app_key = _normalize_app(app)
            if not app_key:
                continue
            duration = _extract_duration(payload_json)
            entry = stats.get(app_key)
            if entry is None:
                entry = AppStats(app=app_key)
                stats[app_key] = entry
            entry.seconds += duration
            entry.blocks += 1
            if ts and ts > entry.last_seen_ts:
                entry.last_seen_ts = ts
    finally:
        conn.close()

    return stats


def _collect_running_exes() -> set[str]:
    names: set[str] = set()
    try:
        output = subprocess.check_output(
            ["tasklist", "/fo", "csv", "/nh"], text=True, errors="ignore"
        )
    except Exception:
        return names
    reader = csv.reader(io.StringIO(output))
    for row in reader:
        if not row:
            continue
        name = row[0].strip().strip('"')
        app_key = _normalize_app(name)
        if app_key:
            names.add(app_key)
    return names


def _collect_installed_exes() -> set[str]:
    names: set[str] = set()
    try:
        import winreg  # type: ignore
    except Exception:
        return names

    uninstall_paths = [
        (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (winreg.HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ]

    for root, subkey in uninstall_paths:
        try:
            with winreg.OpenKey(root, subkey) as key:
                count = winreg.QueryInfoKey(key)[0]
                for idx in range(count):
                    try:
                        child_name = winreg.EnumKey(key, idx)
                        with winreg.OpenKey(key, child_name) as child:
                            display_icon = _read_reg_value(child, "DisplayIcon")
                            uninstall_string = _read_reg_value(child, "UninstallString")
                            quiet_uninstall = _read_reg_value(child, "QuietUninstallString")
                            for raw in (display_icon, uninstall_string, quiet_uninstall):
                                exe = _extract_exe_name(raw)
                                if exe:
                                    names.add(exe)
                    except OSError:
                        continue
        except OSError:
            continue

    return names


def _read_reg_value(key, name: str) -> str:
    try:
        value, _ = _read_reg_value_raw(key, name)
    except OSError:
        return ""
    if value is None:
        return ""
    return str(value)


def _read_reg_value_raw(key, name: str):
    import winreg  # type: ignore

    try:
        return winreg.QueryValueEx(key, name)
    except OSError:
        return ("", 0)


def _extract_exe_name(text: str) -> str:
    if not text:
        return ""
    cleaned = text.strip().strip('"')
    lower = cleaned.lower()
    idx = lower.find(".exe")
    if idx == -1:
        return ""
    path = cleaned[: idx + 4]
    path = path.strip().strip('"')
    exe_name = Path(path).name
    return _normalize_app(exe_name)


def _build_recommendations(
    items: list[AppStats],
    min_minutes: float,
    min_blocks: int,
    allowlist: set[str],
    denylist: set[str],
) -> list[str]:
    output: list[str] = []
    min_minutes = max(0.0, float(min_minutes))
    min_blocks = max(0, int(min_blocks))
    for entry in items:
        if entry.app in allowlist or entry.app in denylist:
            continue
        minutes = entry.seconds / 60.0
        if minutes >= min_minutes or entry.blocks >= min_blocks:
            output.append(entry.app)
    return output


def _sort_candidates(items: list[AppStats]) -> list[AppStats]:
    def score(entry: AppStats) -> tuple[float, int, str]:
        return (entry.seconds, entry.blocks, entry.app)

    return sorted(items, key=score, reverse=True)


def _build_report(
    items: list[AppStats],
    recommended: list[str],
    allowlist: set[str],
    denylist: set[str],
    *,
    start_ts: str | None,
    min_minutes: float,
    min_blocks: int,
) -> Dict[str, Any]:
    return {
        "generated_at": utc_now().isoformat().replace("+00:00", "Z"),
        "window": {"start_ts": start_ts, "end_ts": None},
        "criteria": {
            "min_minutes": float(min_minutes),
            "min_blocks": int(min_blocks),
        },
        "existing": {
            "allowlist_count": len(allowlist),
            "denylist_count": len(denylist),
        },
        "recommended_allow": sorted(recommended),
        "candidates": [
            {
                "app": entry.app,
                "sources": sorted(entry.sources),
                "focus_minutes": round(entry.seconds / 60.0, 2),
                "blocks": entry.blocks,
                "last_seen_ts": entry.last_seen_ts or None,
            }
            for entry in items
        ],
        "selection": {
            "allow": [],
            "deny": [],
        },
    }


def _apply_allowlist_update(
    rules_path: Path,
    allowlist: set[str],
    denylist: set[str],
    additions: Iterable[str],
) -> list[str]:
    merged = []
    for app in additions:
        app_key = _normalize_app(app)
        if not app_key or app_key in allowlist or app_key in denylist:
            continue
        merged.append(app_key)
    if not merged:
        return []

    raw = yaml.safe_load(rules_path.read_text()) or {}
    existing = raw.get("allowlist_apps") or []
    updated = list(existing)
    updated.extend(merged)
    raw["allowlist_apps"] = updated

    backup_path = rules_path.with_suffix(rules_path.suffix + ".bak")
    backup_path.write_text(rules_path.read_text())
    rules_path.write_text(
        yaml.safe_dump(raw, sort_keys=False, allow_unicode=False)
    )
    return merged


def _apply_selection(
    rules_path: Path,
    allowlist: set[str],
    denylist: set[str],
    selected_allow: list[str],
    selected_deny: list[str],
) -> tuple[list[str], list[str]]:
    allow_added = [app for app in selected_allow if app not in allowlist]
    deny_added = [app for app in selected_deny if app not in denylist]
    if not allow_added and not deny_added:
        return ([], [])

    raw = yaml.safe_load(rules_path.read_text()) or {}
    allow_existing = _lower_list(raw.get("allowlist_apps"))
    deny_existing = _lower_list(raw.get("denylist_apps"))
    allow_existing.extend([app for app in allow_added if app not in allow_existing])
    deny_existing.extend([app for app in deny_added if app not in deny_existing])
    raw["allowlist_apps"] = allow_existing
    raw["denylist_apps"] = deny_existing

    backup_path = rules_path.with_suffix(rules_path.suffix + ".bak")
    backup_path.write_text(rules_path.read_text())
    rules_path.write_text(
        yaml.safe_dump(raw, sort_keys=False, allow_unicode=False)
    )
    return allow_added, deny_added


def _load_privacy_lists(path: Path) -> tuple[set[str], set[str]]:
    raw = yaml.safe_load(path.read_text()) or {}
    allowlist = _lower_set(raw.get("allowlist_apps"))
    denylist = _lower_set(raw.get("denylist_apps"))
    return allowlist, denylist


def _lower_set(value: Any) -> set[str]:
    if not value:
        return set()
    if isinstance(value, (list, set, tuple)):
        return {str(item).lower() for item in value}
    return {str(value).lower()}


def _lower_list(value: Any) -> list[str]:
    if not value:
        return []
    if isinstance(value, (list, set, tuple)):
        return [str(item).lower() for item in value]
    return [str(value).lower()]


def _normalize_app(app: Any) -> str:
    if app is None:
        return ""
    app_text = str(app).strip().lower()
    if app_text in {"", "unknown", "none"}:
        return ""
    return app_text


def _extract_duration(payload_json: Any) -> float:
    if not payload_json:
        return 0.0
    try:
        payload = json.loads(payload_json)
    except (TypeError, json.JSONDecodeError):
        return 0.0
    duration = payload.get("duration_sec", 0)
    if isinstance(duration, (int, float)):
        return float(duration)
    try:
        return float(duration)
    except (TypeError, ValueError):
        return 0.0


if __name__ == "__main__":
    main()
