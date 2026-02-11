from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from collector.config import load_config


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Execute automation actions")
    parser.add_argument(
        "--input",
        default="activity_recommendations.json",
        help="recommendations json path",
    )
    parser.add_argument(
        "--config",
        default="",
        help="optional config to load automation defaults",
    )
    parser.add_argument(
        "--allow",
        default="open_app,open_url",
        help="comma-separated allowed actions",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="do not execute, just print",
    )
    parser.add_argument(
        "--approve",
        choices=["prompt", "yes", "no"],
        default="prompt",
        help="approval mode for each action",
    )
    parser.add_argument(
        "--menu",
        action="store_true",
        help="show CLI menu and allow multi-select execution",
    )
    parser.add_argument(
        "--preview",
        action="store_true",
        help="show preview (text + action) before selection",
    )
    parser.add_argument(
        "--preview-template",
        action="store_true",
        help="preview create_file template content in menu",
    )
    parser.add_argument(
        "--schema",
        default=str(PROJECT_ROOT / "schemas" / "recommendations.schema.json"),
        help="path to recommendations schema",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    payload = _load_json(args.input)
    config = load_config(args.config) if args.config else None
    allowed = {item.strip() for item in args.allow.split(",") if item.strip()}
    dry_run = bool(args.dry_run)
    if config and getattr(config, "automation", None):
        if config.automation.allow_actions:
            allowed = set(config.automation.allow_actions)
        dry_run = config.automation.dry_run or dry_run
    if not args.dry_run and not config:
        dry_run = bool(
            os.getenv("AUTOMATION_DRY_RUN", "true").lower() in {"1", "true", "yes"}
        )

    issues = _validate_payload(payload)
    if issues:
        print("invalid_recommendations:")
        for issue in issues:
            print(f"- {issue}")
        return

    items = payload.get("items", [])
    if args.menu:
        items = _menu_select(
            items,
            show_preview=args.preview,
            show_template=args.preview_template,
        )
        if not items:
            print("no_selection")
            return

    min_conf = 0.0
    if config and getattr(config, "automation", None):
        min_conf = float(config.automation.min_confidence or 0.0)

    for item in items:
        action = item.get("action") or {}
        action_type = str(action.get("type") or "")
        target = str(action.get("target") or "")
        confidence = item.get("confidence")
        if confidence is not None and confidence < min_conf:
            print(f"skip action={action_type} target={target} reason=low_confidence")
            continue
        if not action_type or action_type == "none":
            continue
        if action_type not in allowed:
            print(f"skip action={action_type} target={target} reason=not_allowed")
            continue
        if dry_run:
            print(f"dry_run action={action_type} target={target}")
            continue
        if args.approve != "yes" and not _approve(action_type, target, args.approve):
            print(f"skip action={action_type} target={target} reason=user_denied")
            continue
        _execute_action(action_type, target, action)


def _execute_action(action_type: str, target: str, action: dict) -> None:
    if os.name == "nt":
        if action_type == "open_url":
            subprocess.Popen(["cmd", "/c", "start", "", target], shell=False)
            return
        if action_type == "open_app":
            subprocess.Popen([target], shell=False)
            return
        if action_type == "open_path":
            subprocess.Popen(["cmd", "/c", "start", "", target], shell=False)
            return
        if action_type == "create_file":
            _create_file(target, action)
            return
    else:
        if action_type == "open_url":
            subprocess.Popen(["xdg-open", target], shell=False)
            return
        if action_type == "open_app":
            subprocess.Popen([target], shell=False)
            return
        if action_type == "open_path":
            subprocess.Popen(["xdg-open", target], shell=False)
            return
        if action_type == "create_file":
            _create_file(target, action)
            return


def _create_file(target: str, action: dict) -> None:
    if not target:
        return
    path = Path(target)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and not bool(action.get("overwrite")):
        print(f"skip create_file target={target} reason=exists")
        return
    content = ""
    template_path = action.get("template_path")
    if template_path:
        template_file = Path(str(template_path))
        if template_file.exists():
            content = template_file.read_text(encoding="utf-8")
    if not content:
        content = str(action.get("template") or "")
    if not content:
        app_hint = str(action.get("app") or "")
        content = _auto_template(target, app_hint_override=app_hint)
    path.write_text(content, encoding="utf-8")
    print(f"created_file={path}")


def _load_json(path: str) -> Dict[str, Any]:
    p = Path(path)
    if not p.exists():
    return {}


def _validate_payload(payload: dict) -> list[str]:
    issues = []
    if not isinstance(payload, dict):
        return ["payload must be object"]
    if "items" not in payload or not isinstance(payload.get("items"), list):
        return ["items missing or not list"]
    for idx, item in enumerate(payload.get("items", [])):
        if not isinstance(item, dict):
            issues.append(f"item[{idx}] not object")
            continue
        if "text" not in item or "action" not in item:
            issues.append(f"item[{idx}] missing text/action")
            continue
        action = item.get("action") or {}
        if not isinstance(action, dict):
            issues.append(f"item[{idx}].action not object")
            continue
        if "type" not in action:
            issues.append(f"item[{idx}].action.type missing")
        if "target" not in action:
            issues.append(f"item[{idx}].action.target missing")
    return issues


def _approve(action_type: str, target: str, mode: str) -> bool:
    if mode == "yes":
        return True
    if mode == "no":
        return False
    try:
        answer = input(f"Approve action {action_type} -> {target}? (y/N): ").strip().lower()
    except EOFError:
        return False
    return answer in {"y", "yes"}


def _menu_select(
    items: list[dict], *, show_preview: bool, show_template: bool
) -> list[dict]:
    if not items:
        return []
    print("Recommendations:")
    for idx, item in enumerate(items, start=1):
        text = item.get("text", "")
        action = item.get("action") or {}
        action_type = action.get("type", "")
        target = action.get("target", "")
        if show_preview:
            confidence = item.get("confidence")
            confidence_text = f" (confidence={confidence})" if confidence is not None else ""
            print(f"{idx}. {text}{confidence_text}")
            print(f"   action: {action_type} -> {target}")
            if show_template and action_type == "create_file":
                template_text = _preview_template(action, target)
                if template_text:
                    print("   template_preview:")
                    for line in template_text.splitlines():
                        print(f"     {line}")
        else:
            print(f"{idx}. {text} [{action_type} -> {target}]")
    try:
        raw = input("Select items (e.g., 1,3,5 or all): ").strip().lower()
    except EOFError:
        return []
    if raw in {"all", "a"}:
        return items
    selected = []
    parts = [part.strip() for part in raw.split(",") if part.strip()]
    for part in parts:
        if not part.isdigit():
            continue
        idx = int(part) - 1
        if 0 <= idx < len(items):
            selected.append(items[idx])
    return selected


def _auto_template(target: str, app_hint_override: str = "") -> str:
    templates_dir = PROJECT_ROOT / "templates"
    name = Path(target).name.lower()
    ext = Path(target).suffix.lower()

    app_hint = (app_hint_override or Path(target).stem or "").lower()

    candidates = []
    if "notion" in app_hint:
        candidates.append("notion.md")
    if "vscode" in app_hint or "code" in app_hint:
        candidates.append("vscode.md")
    if "chrome" in app_hint:
        candidates.append("chrome.md")
    if "report" in name:
        candidates.append("report.md")
    if "meeting" in name:
        candidates.append("meeting.md")
    if "summary" in name:
        candidates.append("summary.md")
    if ext in {".md", ".txt"}:
        candidates.append("default.md")
    candidates.append("default.md")

    for filename in candidates:
        path = templates_dir / filename
        if path.exists():
            return path.read_text(encoding="utf-8")
    return ""


def _preview_template(action: dict, target: str) -> str:
    template_path = action.get("template_path")
    if template_path:
        path = Path(str(template_path))
        if path.exists():
            return _summarize_template(path.read_text(encoding="utf-8"))
    template_text = action.get("template") or ""
    if template_text:
        return _summarize_template(str(template_text))
    app_hint = str(action.get("app") or "")
    auto_text = _auto_template(target, app_hint_override=app_hint)
    return _summarize_template(auto_text)


def _summarize_template(text: str, max_lines: int = 6) -> str:
    if not text:
        return ""
    lines = text.splitlines()
    snippet = lines[:max_lines]
    if len(lines) > max_lines:
        snippet.append("...")
    return "\n".join(snippet)
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {}


if __name__ == "__main__":
    main()
